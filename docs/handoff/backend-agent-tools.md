# 에이전트 도구 5개 — 붙여 넣을 코드(백엔드 → 리드)

백엔드 세션은 `crates/app/*`를 직접 고칠 수 없어(작업 범위 규칙), `crates/app/gc_agent/src/exec.rs`에 넣을 코드를 여기 둔다.
모두 기존 세션 API 위에 있으며 이름으로 대상을 찾는다(부품·로봇 링크는 `상위/이름`).

## `Call`에 추가할 변형

```rust
    /// The assembly tree: components, their parts and robot links, with their states, constraint status
    /// and the freedom left.
    Components,
    /// A component's origin in the world: centre point and axes.
    ComponentOrigin { component: String },
    /// Adds an assembly constraint or joint between origin elements of two components; the first moves.
    /// kind: mate | flush | angle | rigid | rotational | slider | cylindrical | ball;
    /// elements: xy | xz | yz | x | y | z | origin; value: mm (mate, flush) or degrees (angle).
    Constrain { kind: String, first: String, first_element: String, second: String, second_element: String, value: f64, flip: bool },
    /// Moves components to the assembly's origin, aligned with it, and grounds them.
    GroundAtOrigin { components: Vec<String> },
    /// Switches grounded | visible | enabled | suppressed on a component.
    SetComponentState { component: String, state: String, on: bool },
```

`edits()`에서 `Components`, `ComponentOrigin`은 읽기 전용.

## 도우미

```rust
fn named_components(session: &Session) -> Vec<(String, gc_doc::ComponentNode)> {
    let mut nodes = session.components();
    for (id, _) in session.doc().robots().iter() {
        nodes.extend(session.robot_links(id));
    }
    let top = |o: gc_doc::Occurrence| {
        nodes.iter().find(|n| n.occurrence == gc_doc::Occurrence::Item(o.owner())).map(|n| n.name.clone())
    };
    nodes
        .iter()
        .map(|n| {
            let name = match n.occurrence {
                gc_doc::Occurrence::Item(_) => n.name.clone(),
                other => format!("{}/{}", top(other).unwrap_or_default(), n.name),
            };
            (name, n.clone())
        })
        .collect()
}

fn component_named(session: &Session, name: &str) -> Result<gc_doc::Occurrence, String> {
    named_components(session)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, node)| node.occurrence)
        .ok_or_else(|| format!("no component named `{name}`; call components for the names"))
}

fn element_geometry(element: &str, joint: bool) -> Result<gc_doc::Geometry, String> {
    use gc_doc::{Geometry, OriginElement, OriginPlane};
    Ok(match element {
        "xy" => Geometry::origin(OriginElement::Plane(OriginPlane::Xy)),
        "xz" => Geometry::origin(OriginElement::Plane(OriginPlane::Xz)),
        "yz" => Geometry::origin(OriginElement::Plane(OriginPlane::Yz)),
        "x" => Geometry::origin(OriginElement::Axis(0)),
        "y" => Geometry::origin(OriginElement::Axis(1)),
        "z" => Geometry::origin(OriginElement::Axis(2)),
        "origin" if joint => Geometry::origin_frame(),
        "origin" => Geometry::origin(OriginElement::Point),
        other => return Err(format!("`{other}` is not xy, xz, yz, x, y, z or origin")),
    })
}
```

## `execute`의 갈래

```rust
        Call::Components => Ok(json!(named_components(session)
            .into_iter()
            .map(|(name, node)| {
                let item = match node.occurrence { gc_doc::Occurrence::Item(i) => Some(i), _ => None };
                json!({
                    "name": name,
                    "grounded": node.grounded, "visible": node.visible,
                    "enabled": node.enabled, "suppressed": node.suppressed,
                    "constraint_status": item.and_then(|i| session.component_status(i)).map(|s| format!("{s:?}").to_lowercase()),
                    "freedom": item.and_then(|i| session.component_dof(i)).map(|d| json!({ "total": d.total, "translation": d.translation })),
                })
            })
            .collect::<Vec<_>>())),
        Call::ComponentOrigin { component } => {
            let origin = session.component_origin(component_named(session, component)?).ok_or("the component has no origin")?;
            let axis = |i: usize| origin.axis(i).to_array().map(|v| (v * 1e6).round() / 1e6);
            Ok(json!({ "point_mm": origin.point().to_array().map(mm), "x_axis": axis(0), "y_axis": axis(1), "z_axis": axis(2) }))
        }
        Call::Constrain { kind, first, first_element, second, second_element, value, flip } => {
            use gc_doc::{AngleSolution, AxisSense, RelationKind, Relationship, Selection};
            if !value.is_finite() { return Err("the value must be a finite number".to_owned()); }
            let (label, kind) = match kind.as_str() {
                "mate" => ("Mate", RelationKind::Mate { offset: value / 1000.0, sense: AxisSense::Opposed }),
                "flush" => ("Flush", RelationKind::Flush { offset: value / 1000.0 }),
                "angle" => ("Angle", RelationKind::Angle { angle: value.to_radians(), solution: AngleSolution::Directed }),
                "rigid" => ("Rigid", RelationKind::Rigid { flip: *flip }),
                "rotational" => ("Rotational", RelationKind::Rotational { flip: *flip }),
                "slider" => ("Slider", RelationKind::Slider { flip: *flip }),
                "cylindrical" => ("Cylindrical", RelationKind::Cylindrical { flip: *flip }),
                "ball" => ("Ball", RelationKind::Ball),
                other => return Err(format!("`{other}` is not a constraint or joint kind")),
            };
            let joint = matches!(label, "Rigid" | "Rotational" | "Slider" | "Cylindrical" | "Ball");
            let pick = |name: &str, element: &str| -> Result<Selection, String> {
                Ok(Selection { occurrence: component_named(session, name)?, geometry: element_geometry(element, joint)? })
            };
            let (a, b) = (pick(first, first_element)?, pick(second, second_element)?);
            let name = format!("{label}:{}", session.relationships().len() + 1);
            session.add_relationship(Relationship::new(name.clone(), kind, a, b)).map_err(edit_error)?;
            Ok(json!({ "relationship": name }))
        }
        Call::GroundAtOrigin { components } => {
            let items: Vec<gc_core::AnyId> =
                components.iter().map(|c| component_named(session, c).map(|o| o.owner())).collect::<Result<_, _>>()?;
            session.ground_at_origin(&items).map_err(edit_error)?;
            Ok(json!({ "grounded": components }))
        }
        Call::SetComponentState { component, state, on } => {
            use gc_doc::ComponentFlag;
            let flag = match state.as_str() {
                "grounded" => ComponentFlag::Grounded, "visible" => ComponentFlag::Visible,
                "enabled" => ComponentFlag::Enabled, "suppressed" => ComponentFlag::Suppressed,
                other => return Err(format!("`{other}` is not grounded, visible, enabled or suppressed")),
            };
            session.set_component(component_named(session, component)?, flag, *on).map_err(edit_error)?;
            Ok(json!({ "component": component, state.as_str(): on }))
        }
```

## 시험(tests/agent.rs에 추가할 시나리오)

바디 두 개(`add_body`) → `Components`에 두 이름과 첫 바디 `grounded: true` → `Constrain{kind:"mate", first:"Body2", first_element:"xy",
second:"Body1", second_element:"xy", value:0, ..}`는 XY 평면 Mate라 법선이 반대가 되어 뒤집힘; `"flush"`면 Body2 원점 z가 Body1과 같음
(`ComponentOrigin`의 `point_mm`으로 확인) → `Components`의 Body2 `freedom.total == 3` → `SetComponentState{state:"suppressed"}` →
`Undo` 두 번으로 복귀. 로봇 링크 이름은 `UR5e_1/wrist_3_link` 꼴.
