//! Offscreen render target and pixel readback.

use crate::{COLOR_FORMAT, DEPTH_FORMAT, MSAA_SAMPLES};

/// Errors raised by the renderer.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The device did not finish the work in time or was lost.
    #[error("GPU poll failed: {0}")]
    Poll(String),
    /// Mapping the readback buffer failed.
    #[error("readback buffer could not be mapped: {0}")]
    Map(String),
}

/// Colour (resolved + multisampled) and depth attachments of one viewport.
pub struct ViewTarget {
    width: u32,
    height: u32,
    resolve: wgpu::Texture,
    resolve_view: wgpu::TextureView,
    msaa_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

impl ViewTarget {
    /// Creates the attachments. Sizes are clamped to at least one pixel.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = |label, format, sample_count, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let resolve = texture(
            "gc_render.resolve",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let msaa =
            texture("gc_render.msaa", COLOR_FORMAT, MSAA_SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let depth =
            texture("gc_render.depth", DEPTH_FORMAT, MSAA_SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let view = |t: &wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            width,
            height,
            resolve_view: view(&resolve),
            msaa_view: view(&msaa),
            depth_view: view(&depth),
            resolve,
        }
    }

    /// Size in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// The single-sample `Rgba8Unorm` view a UI toolkit displays.
    pub fn resolve_view(&self) -> &wgpu::TextureView {
        &self.resolve_view
    }

    /// Begins the main pass: clears colour and depth and resolves into [`ViewTarget::resolve_view`].
    pub fn begin_pass<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        clear: wgpu::Color,
    ) -> wgpu::RenderPass<'e> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gc_render.main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.msaa_view,
                depth_slice: None,
                resolve_target: Some(&self.resolve_view),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }
}

/// Clears the target to one colour (linear RGBA in 0..=1) and submits the work.
pub fn clear(device: &wgpu::Device, queue: &wgpu::Queue, target: &ViewTarget, color: [f64; 4]) {
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("gc_render.clear") });
    let pass =
        target.begin_pass(&mut encoder, wgpu::Color { r: color[0], g: color[1], b: color[2], a: color[3] });
    drop(pass);
    queue.submit([encoder.finish()]);
}

/// Reads the resolved colour texture back as tightly packed RGBA8 rows (top row first).
pub fn read_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &ViewTarget,
) -> Result<Vec<u8>, RenderError> {
    let (width, height) = target.size();
    let unpadded = 4 * width;
    let padded = unpadded.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gc_render.readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("gc_render.readback") });
    encoder.copy_texture_to_buffer(
        target.resolve.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    let submission = queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
        // The receiver only disappears if this function already returned with an error.
        drop(tx.send(result));
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .map_err(|e| RenderError::Poll(e.to_string()))?;
    rx.recv().map_err(|e| RenderError::Map(e.to_string()))?.map_err(|e| RenderError::Map(e.to_string()))?;
    let mapped = buffer.slice(..).get_mapped_range().map_err(|e| RenderError::Map(e.to_string()))?;
    let mut pixels = Vec::with_capacity((unpadded * height) as usize);
    for row in mapped.chunks(padded as usize) {
        if let Some(visible) = row.get(..unpadded as usize) {
            pixels.extend_from_slice(visible);
        }
    }
    Ok(pixels)
}
