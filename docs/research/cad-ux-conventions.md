# GraphCTRL: Inventor / SolidWorks UX conventions for Milestone 1

**Evidence.** Direct page fetches failed: `help.autodesk.com`, `help.solidworks.com` and `blogs.solidworks.com` are outside this session's allowed network domains and must be allowed before full pages can be read. Tags:
- `V` = confirmed today from a search-engine excerpt of the cited vendor page.
- `V2` = confirmed from a secondary source (reseller, forum).
- `R` = recalled from memory, not verified today.

No files were read, created or modified.

## 1 + A. Navigation presets

### Inventor

| Action | Input | Tag |
|---|---|---|
| Pan | MMB drag; hold F2 + LMB drag | V I4, I1 |
| Zoom | Wheel; hold F3 + LMB drag | V I4, I1 |
| Wheel direction | Default: roll toward user = zoom in. Option "Reverse direction" flips it | V I5 |
| Zoom anchor | Option "Zoom to cursor" (else display centre); on by default | option V2 I21, default R |
| Free orbit | Shift+MMB drag; hold F4 + LMB drag (releasing F4 resumes the interrupted command) | V I4, I8 |
| Orbit modes | Free = screen-relative; Constrained = turntable about the ViewCube Top-Bottom axis; option "Default Orbit Type" | V I5, I8 |
| Orbit reticle | Drag inside = tumble; side handles = about vertical axis; top/bottom handles = about horizontal axis | V I8 (wording from the assembly Rotate command) |
| Orbit reticle (rest) | Drag outside = roll; click = new centre | R |
| Pivot | 2024.1+: "Use nearest model point for Orbit", default on; earlier: view centre | V2 I7 |
| Remapping | 2022+: MMB, Ctrl+MMB, Shift+MMB each assignable to Pan / Zoom / Orbit | V I6 |
| Zoom All | Home; double-click MMB | V I3; R |
| Zoom Window | Shift+F3 | V I2 |
| Zoom Selected | End (another Autodesk page disagrees) | V I3 |
| Look At (normal to) | Page Up, then pick face / edge / sketch | V I3 |
| Previous / Next view | F5 / Shift+F5 | V I1, I2 |
| Home (isometric) view | F6 | V I3 |
| Standard views while orbiting | Space | V I2 |
| Slice graphics at sketch plane | F7 | V I2 |
| Projection | ViewCube menu: Orthographic, Perspective, Perspective with Ortho Faces | V I9 |

ViewCube (top right):
- Faces, edges and corners are click targets (V I9; 26 in total, R).
- With a face view current, triangles jump to adjacent faces (V I9); two curved arrows roll 90° (R).
- Dragging the cube orbits; a Home icon appears on hover (R).
- Menu (V I9): Set Current View as Home > Fixed Distance | Fit to View; Set Current View as Front; Reset Front; Set Current View as > Top | Front.

### SolidWorks

| Action | Input | Tag |
|---|---|---|
| Rotate | MMB drag; arrow keys (step set by the "Arrow keys" option, default 15°) | V S1, S3; 15° R |
| Rotate 90° | Shift+arrows | V S2 |
| Rotate about entity | MMB-click a vertex / edge / face, then MMB drag; not while editing a sketch | V S1, S4 |
| Roll | Alt+MMB drag; Alt+Left / Right | V S2, S4 |
| Pan | Ctrl+MMB drag; Ctrl+arrows (in drawings plain MMB drag) | V S1, S2 |
| Zoom | Wheel, anchored at pointer (model centre if pointer is outside the viewport); Shift+MMB drag; Z = out, Shift+Z = in | V S1, S2 |
| Wheel direction | Default: roll toward user = zoom in; option "Reverse mouse wheel zoom direction" | option V S3, default R |
| Zoom to Fit | F; double-click MMB | V S2; V2 |
| Standard views | Ctrl+1 Front, 2 Back, 3 Left, 4 Right, 5 Top, 6 Bottom, 7 Isometric, 8 Normal To | V2 (1 and 8), rest R |
| Orientation dialog + View Selector | Space; View Selector only: Ctrl+Space; Alt+click picks back faces | V S2 |
| Previous view | Ctrl+Shift+Z | R |
| Magnifying glass | G | R |
| Fit on view change | Option "Zoom to fit when changing to standard views" | V S3 |
| Triad (bottom left) | Click axis = look along it; click again = flip 180° | V2 S4 |
| Triad modifiers | Shift+click = 90° about axis; Alt+click = one arrow-step | R |
| Zoom to Area / Selection, Perspective | Heads-up toolbar / View menu, no default key | R |

### Up axis and standard views
- SolidWorks is Y-up by default. The Orientation dialog has an "Up Axis" flyout (Apply Y-up / Z-up views, since 2020), and 2025 offers Z-up templates. Only views change; plane names do not (V S5).
- Inventor has no Z-up switch. You redefine ViewCube Front/Top and Home, then save the template (V2 I19).
- Derived camera table (R); Home/Isometric is 45° azimuth, 35.264° elevation:

| View | Y-up: look, up | Z-up: look, up |
|---|---|---|
| Front | −Z, +Y | +Y, +Z |
| Right | −X, +Y | −X, +Z |
| Top | −Y, −Z | −Z, +Y |
| Iso | (−1,−1,−1), +Y | (−1,+1,−1), +Z |

## 2. UI layout (mostly R)

| Region | Inventor | SolidWorks |
|---|---|---|
| Top | File menu; Quick Access Toolbar (undo/redo, Home, Update, selection priority, fx Parameters); ribbon tabs 3D Model, Sketch, Annotate, Inspect, Tools, Manage, View | Menu bar (File … Help); CommandManager tabs Features, Sketch, Evaluate … |
| Left | Browser: part node, Solid Bodies, Origin {YZ, XZ, XY Plane; X, Y, Z Axis; Center Point}, Sketch nodes, End of Part | FeatureManager design tree {Front / Top / Right Plane, Origin, features, rollback bar}; PropertyManager replaces it while a tool runs (green check / red X); ConfigurationManager |
| Viewport | ViewCube; Navigation bar (wheel, Pan, Zoom flyout, Orbit, Look At; V I22); axis indicator; document tabs; heads-up input | Heads-up View toolbar; triad; confirmation corner top right (Exit Sketch / cancel); context toolbars on selection |
| Right | none | Task Pane |
| Bottom | Prompt text left; cursor coordinates and "N dimensions needed" right; sketch toggles (constraints, slice, DOF, Relax Mode) | Prompt; coordinates; sketch status; "Editing Sketch1"; unit system |
| Quick access | Marking menu: RMB radial, flick to execute | S = shortcut bar at cursor; RMB-drag mouse gestures |

Inventor Sketch tab panels (R; V I18 for Create > Line/Text, Insert > Image, Format > Construction, Finish Sketch):
- **Create**: Line, Circle, Arc, Rectangle, Spline, Ellipse, Slot, Polygon, Point, Fillet/Chamfer, Text, Project Geometry.
- **Modify**: Move, Copy, Rotate, Trim, Extend, Split, Scale, Stretch, Offset.
- **Pattern**: Rectangular, Circular, Mirror.
- **Constrain**: Dimension, Automatic Dimensions and Constraints, Show Constraints, 12 constraints, Constraint Settings.
- **Insert**, **Format** (Construction, Centerline, Center Point, Driven Dimension), **Exit** (Finish Sketch).

SolidWorks Sketch tab (R): Sketch/Exit, Smart Dimension, Line/Centerline, Rectangle, Slot, Circle, Arc, Polygon, Spline, Ellipse, Fillet, Point, Trim/Extend, Convert Entities, Offset, Mirror, Patterns, Move, Display/Delete Relations (Add Relation, Fully Define Sketch), Repair Sketch, Quick Snaps.

## 3. Sketch workflow

| Tool | Inventor | SolidWorks |
|---|---|---|
| Start / finish | S (V I2) or Start 2D Sketch, pick plane/face; view turns to the plane and origin point is auto-projected (R). Finish Sketch or RMB > Finish 2D Sketch (V I23) | Sketch, pick plane/face; exit via confirmation corner (R) |
| Line | L (V I2). Click-click chain; press-drag from the last endpoint = tangent or perpendicular arc (V I15). Double-click / Enter ends chain; Esc quits (R) | Click-click chains, press-drag makes one segment; double-click or RMB > End chain keeps tool; Esc quits; arc by returning to the endpoint or pressing A (V S11). Key L (R) |
| Circle | C = Center Point Circle (V I1); Tangent Circle | Circle; Perimeter Circle (3 points) |
| Arc | Three Point (start, end, bulge), Tangent, Center Point (centre, start, end) | 3 Point, Tangent, Centerpoint |
| Rectangle | Two Point, Three Point, Two / Three Point Center | Corner, Center, 3 Point Corner / Center, Parallelogram |
| Other | Point, Spline, Ellipse, Slot, Polygon | same |
| Modify | Fillet/Chamfer; Trim (alias TR, V I2; X, R): hover preview, click removes to nearest intersections; Extend; Split; Offset | Sketch Fillet; Trim (Power, Corner, Inside, Outside, Closest); Extend; Offset |
| Copy / reference | Mirror, patterns; Project Geometry | Mirror Entities, patterns; Convert Entities |
| Construction | Format > Construction / Centerline (V I18) | Construction Geometry toggle; Centerline tool |
| Numeric entry | Heads-Up Display: Pointer Input, Dimension Input (length / angle), Tab between fields, optional persistent dimensions (V I14) | "Enable on screen numeric input on entity creation"; dimension created on Enter/Tab (V S12) |

## 4. Inference and snaps
- **Inventor** (V I11): constraints are inferred and applied while sketching, and Ctrl suppresses them. "Persistence" keeps them and is disabled when inference is off. Endpoint coincidence is always inferred unless switched off. Constraint Settings > Inference tab holds the defaults.
- **Inventor display** (R): glyph at the cursor, dotted alignment lines, and hovering a curve makes it the parallel/perpendicular reference.
- **SolidWorks** (V S8): dotted inferencing lines; the pointer shows the pending relation (horizontal, vertical, coincident, midpoint, perpendicular, tangent). Tools > Sketch Settings > Automatic Relations toggles it, Ctrl suppresses it, and Quick Snaps are on the RMB menu.
- **SolidWorks colours** (R): a yellow line means a relation will be added; blue means alignment only.
- **Snap types** (R): endpoint, centre, midpoint, quadrant, intersection, nearest, tangent, perpendicular, parallel, H/V, grid, angle.
- **Priority**: neither vendor documents one. Proposed: point > centre > midpoint > intersection > quadrant > tangent/perpendicular > on-curve > alignment > grid.

## 5. Constraint catalogue

| GraphCTRL | Inventor | SolidWorks (V S7) | Selection and meaning |
|---|---|---|---|
| Coincident | Coincident | Coincident; Merge Points | point+point; point on infinite line / full circle |
| Collinear | Collinear | Collinear | 2+ lines |
| Concentric | Concentric | Concentric; Coradial (also equal radius) | arcs / circles |
| Fix | Fix | Fix (endpoints may still slide along a fixed line) | any |
| Parallel, Perpendicular | same | same | 2 lines |
| Horizontal, Vertical | same | same | line(s) or 2+ points |
| Tangent | Tangent | Tangent | line / arc / ellipse / spline pairs |
| Smooth | Smooth (G2) (V I12) | Equal Curvature (R) | spline + adjoining curve |
| Symmetric | Symmetric | Symmetric | 2 entities + centreline |
| Equal | Equal | Equal | line lengths; radii |
| Midpoint | via snap | Midpoint | point + line |
| Intersection, Pierce | none | Intersection, Pierce | point + 2 lines; point + 3D curve |

The Inventor glossary lists coincident, collinear, concentric, equal, fix, horizontal, parallel, perpendicular, tangent, vertical (V I24).

Managing constraints:
- **Inventor**
  - F8 shows all constraints, F9 hides all (V I1). Hovering a glyph highlights its partners and Delete removes it (R).
  - Relax Mode (F11 or status bar, V I12): dragging or adding a conflicting item deletes the conflicting constraints and makes conflicting dimensions driven. Coincident, Tangent, Symmetric, Smooth and equation-driven dimensions are never relaxed.
  - Automatic Dimensions and Constraints and Sketch Doctor exist (R). Degrees-of-freedom glyphs can be shown (V I10).
- **SolidWorks**
  - View > Hide/Show > Sketch Relations toggles the callouts (V S15).
  - Display/Delete Relations filters: All in this sketch, Dangling, Over Defining/Not Solved, External, Defined In Context, Locked, Broken, Selected Entities (V S15).
  - Fully Define Sketch computes the missing dimensions and relations (V S9). SketchXpert offers solution sets for over-defined sketches (V S15).

## 6. Dimensions
Both use one tool: Inventor Dimension (key D, V I1) and SolidWorks Smart Dimension.

| Selection | Result |
|---|---|
| Line | Length; for an inclined line, cursor position chooses aligned / horizontal / vertical (R) |
| 2 points | Horizontal / vertical / aligned by placement (V S10) |
| 2 parallel lines | Distance |
| 2 other lines | Angle; cursor quadrant picks which one |
| Point + line | Perpendicular distance (V S10) |
| Circle | Diameter; SW shows linear or diameter by placement (V S10); Inventor RMB switches to radius (V2) |
| Arc | Radius; arc length in SW = arc + both endpoints (V S10); Inventor RMB > Arc Length (R) |
| Arc/circle + other | Measured to the centre even if the rim is picked (V S10) |
| Centreline + line/point | Linear diameter (R) |

- **Editing**: the box opens on placement (Inventor option "Edit dimension when created", V I13); double-click edits later (V I25).
- **Inventor parameters** (R): names d0, d1, …; input such as `10 mm`, `d0/2`. Clicking another dimension inserts its name. The fx Parameters table holds Model / Reference / User parameters.
- **SolidWorks parameters** (R): names like `D1@Sketch1`; `=` starts an equation; global variables are quoted.
- **Driven dimensions**: Inventor shows them in parentheses and they are not editable (V I10). SolidWorks shows them gray, with RMB > Driven to toggle (V S6, S9).
- **Inventor over-constraint prompt**: "Adding this dimension will over-constrain the sketch. Choose Accept to create…" (V2 I13, truncated forum title; the remainder "…a Driven Dimension" is R), with Accept / Cancel (V I10). The setting "Over-constrained Dimensions" chooses between applying a driven dimension and warning (V I13).
- **SolidWorks over-constraint prompt**: the "Make Dimension Driven?" dialog offers Make This Dimension Driven / Leave This Dimension Driving, Cancel, and "Don't ask again" (V S9).

## 7. Status feedback
SolidWorks states (V S6):

| State | Colour | Meaning |
|---|---|---|
| Under Defined | blue | needs a dimension or relation |
| Fully Defined | black | all positions determined |
| Over Defined | red | conflicting or redundant items |
| Item Conflicts | yellow | redundant dimension / unnecessary relation |
| No Solution Found | pink in 2016–2019 help | not solved |
| Invalid Solution Found | yellow | zero-length line, zero-radius arc, self-intersecting spline |
| Dangling | brown; olive from 2025 (V S15) | lost reference |
| Driven | gray | reference value |

- **SolidWorks**: the whole-sketch state appears in the status bar. Tree prefixes are (-) under, (+) over (R).
- **Inventor**:
  - The status bar shows the number of dimensions still required (V I10); the strings are "N dimensions needed" / "Fully Constrained" (R).
  - Colours depend on the scheme. Old defaults went green to black; a current default is reported as black to very dark blue and "hard to distinguish" (V2 I20).
- **Dragging**: under-defined geometry follows the cursor with constraints live; fully defined geometry does not move (R).

## 8. 3D sketch (summary)
- **SolidWorks** (V S13)
  - Tab cycles the XY / YZ / ZX plane after picking a tool; a space handle at the first point selects the axis.
  - Relations: AlongX/Y/Z, ParallelYZ/ZX, On Plane, On Surface, and Horizontal / Vertical / Normal on a local plane.
  - Dimensions are true length; Tab cycles along-axis variants (R).
- **Inventor** (V I17)
  - Toggles: Ortho Mode (Ctrl+R), Dynamic Dimension, Snap Object, Infer Constraints (Ctrl+I, V I3).
  - Constraints: Parallel with X/Y/Z Axis, Parallel with XY/YZ/XZ Plane (V I26), On Face, plus the usual ones.
  - Points are placed by triad / precise input and model snaps (R).
- **For robot paths** you need: 3D points, lines, arcs and splines; along-axis, on-plane, on-face and coincident-to-model constraints; true-length dimensions.

## 9. Selection and misc
- **Click selection** (R): Ctrl+click toggles in both; Shift+click adds.
- **Box selection**: left-to-right selects enclosed items with a solid box; right-to-left selects crossing items with a dotted box. SolidWorks also has a lasso (V S14). Inventor follows the same convention (R).
- **Filters**: SolidWorks F5 shows the selection filter toolbar (V S14). Inventor uses Shift+RMB and a priority dropdown (R).
- **Hover and keys** (R): everything pickable pre-highlights; Esc cancels, Enter / RMB OK accepts, Enter repeats the last command, Delete removes.
- **Undo** (R): undo is per command, and view history is a separate stack.
- **Units and grid** (R): mm and degrees; snap-to-grid off; SolidWorks grid hidden by default; Inventor's grid default is unverified.

## B. MVP priorities
**Must have**
1. Shell: tabbed ribbon, browser with Origin (3 planes, 3 axes, centre point) and sketch nodes, view cube, status bar with prompt and DOF counter.
2. Navigation: both presets as data, zoom to cursor, pivot handling, animated standard views, look-at, fit / window / selected, previous / next view, ortho and perspective.
3. Sketch lifecycle: pick plane, auto look-at, finish; projected origin point.
4. Entities: point, chained line with tangent-arc gesture, centre circle, 3-point and centre arcs, 2-point rectangle, construction toggle.
5. Inference with Ctrl suppress; glyphs; F8 / F9.
6. Constraints: the 12 Inventor types plus Midpoint.
7. Smart dimension: linear in three orientations, point-line, parallel distance, angle, radius, diameter; edit box; expressions with units and references; driven dimensions; over-constraint prompt.
8. Per-entity DOF colouring, live drag solve, Trim, Delete, undo/redo, hover highlight, window / crossing selection.

**Later**: slots, polygon, ellipse, spline, G2, offset, mirror, patterns, fillet, extend / split, project geometry, linear-diameter and arc-length dimensions, heads-up numeric input, Relax Mode, auto-dimension, sketch diagnosis, relations manager panel, parameters table, marking menu / shortcut bar, selection filters, lasso, 3D sketch.

## C. Behaviours experts notice when missing
- Zoom anchored exactly under the cursor; pan keeps the grabbed point under the cursor.
- Orbit pivot does not jump at drag start; view changes animate and keep the model upright.
- The view turns to the sketch plane on entry.
- Line chaining with rubber-band preview; Esc versus double-click semantics; tangent-arc gesture.
- The inferred constraint is shown before the click; Ctrl suppresses it.
- The dimension tool stays active; the value box opens pre-selected; Enter commits.
- Dimension type follows the cursor while placing.
- Colour and DOF counter update instantly; fully constrained geometry refuses to drag.
- Over-constraint is never silent.
- Glyph hover highlights participants; Delete works on glyphs.
- Points win over curves in picking, with a pixel tolerance.
- Solid versus dashed selection box.
- View changes are not on the undo stack.
- Esc always backs out one level.

## D. Default colours (approximations from memory, not sampled; R)

| Item | SolidWorks-like | Inventor-like |
|---|---|---|
| Under-defined | #0000FF | #2E8B3D (legacy green) |
| Fully defined | #000000 | #2B2D7A |
| Over-defined / unsolved | #FF0000 | #FF0000 |
| Conflict / invalid | #FFD400 | #FFD400 |
| Dangling | #808000 | #808000 |
| Driven dimension | #808080 | same as geometry, in parentheses |
| Hover / selected | #FF8000 / #3FA9F5 | #6FC3FF / #0696D7 |
| Background gradient (top to bottom) | #C9D5E6 to #F5F7FA | #E6EAEE to #C3CAD3 |

## Sources
**Inventor**
- I1 https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-DEEB0723-DFC7-4B28-95F4-099CC1E1EBF8.htm
- I2 https://help.autodesk.com/cloudhelp/2024/ENU/Inventor-Help/files/GUID-428B9A8E-E513-44C8-888A-F6048AE89F8E.htm
- I3 https://www.autodesk.com/shortcuts/inventor
- I4 https://help.autodesk.com/cloudhelp/2014/ENU/Inventor/files/GUID-77AFD820-CF7D-4E1C-90D8-6B76AC75D70B.htm
- I5 https://help.autodesk.com/cloudhelp/2014/ENU/Inventor/files/GUID-8315A826-9154-47FB-8E34-BA95CAC547E2.htm
- I6 https://blogs.autodesk.com/inventor/?p=2110
- I7 https://resources.imaginit.com/manufacturing/inventor-2024-turn-off-orbit-pivot-point
- I8 https://help.autodesk.com/cloudhelp/2014/ENU/Inventor/files/GUID-5E6B1A06-1043-45C4-A3C3-F0959939EB3B.htm
- I9 https://help.autodesk.com/cloudhelp/2024/ENU/Inventor-Help/files/GUID-285B9036-0E97-4F18-A6D2-EF78B8B8FAA9.htm
- I10 https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-FAF69614-E8F2-4763-975C-552E0BEA1DD1.htm
- I11 https://help.autodesk.com/cloudhelp/2014/ENU/Inventor/files/GUID-1E504789-DFC6-4B59-81AA-1EFC1B656FB1.htm
- I12 https://help.autodesk.com/cloudhelp/2025/ENU/Inventor-Help/files/GUID-B06FA7B7-5CD5-4D58-B1C5-0C5ABAFFD84E.htm
- I13 https://help.autodesk.com/cloudhelp/2015/ENU/Inventor-Help/files/GUID-6DFAA88A-0C90-429E-A694-AFF93730C095.htm and https://forums.autodesk.com/t5/inventor-forum/adding-this-dimension-will-over-constrain-the-sketch-choose/m-p/8193612
- I14 https://help.autodesk.com/cloudhelp/2021/ENU/Inventor-Help/files/GUID-33A8B11D-A04C-4809-BC82-C0419215915F.htm
- I15 https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-4F861674-F74C-474E-869A-4C190ED6EC4B.htm
- I17 https://help.autodesk.com/cloudhelp/2018/ENU/Inventor-WhatsNew/files/GUID-DCE06C79-C8B7-45FF-A42C-91A2F167D548.htm
- I18 https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-F5A5790A-F748-49F0-A81F-BCA56C7C1E89.htm
- I19 https://ukcommunity.arkance.world/hc/en-us/articles/21551432187794-Inventor-How-to-change-the-Z-axis-to-point-up and https://forums.autodesk.com/t5/inventor-forum/why-inventor-do-not-add-feature-change-z-axis-up/m-p/9654915
- I20 https://eng-tips.com/threads/customizable-sketching-colors.452779
- I21 https://forums.autodesk.com/t5/inventor-forum/restore-zoom-tool-to-using-cursor-as-center-point-instead-of/m-p/6892283
- I22 https://help.autodesk.com/cloudhelp/2026/ENU/Inventor-Help/files/GUID-E13A932A-6102-46E9-A132-05F205689777.htm
- I23 https://help.autodesk.com/cloudhelp/2017/ENU/Inventor-Help/files/GUID-E96FB3C4-136B-4551-BF45-F54594543ECB.htm
- I24 https://help.autodesk.com/cloudhelp/2016/ENU/Inventor-Help/files/GUID-3E0794C5-75B4-4425-8163-45FCCF8597B3.htm
- I25 https://help.autodesk.com/cloudhelp/2024/ENU/Inventor-Help/files/GUID-5E25D5FB-3B94-44A0-AD50-80EF06F5A24F.htm
- I26 https://help.autodesk.com/view/INVNTOR/2024/ENU/?guid=GUID-A85A7A30-7D81-4C75-8769-CAD034EEA930

**SolidWorks**
- S1 https://help.solidworks.com/2024/English/SolidWorks/sldworks/r_Middle_Mouse_Button.htm
- S2 https://help.solidworks.com/2023/english/solidworks/sldworks/c_time_saving_keyboard_shortcuts.htm
- S3 https://help.solidworks.com/2026/english/solidworks/sldworks/HIDD_OPTIONS_VIEW_ROTATION.htm
- S4 https://blogs.solidworks.com/products/solidworks/how-do-i-manipulate-my-model-view-let-me-count-the-ways/
- S5 https://help.solidworks.com/2020/English/WhatsNew/t_view_up_axis.htm and https://help.solidworks.com/2025/English/WhatsNew/c_wn2025_fundamentals_specifying_z_up_template.htm
- S6 https://help.solidworks.com/2024/English/SolidWorks/sldworks/c_Sketch_Geometry_Status.htm and https://help.solidworks.com/2019/English/SolidWorks/acadhelp/c_Sketch_States_AcadHelp.htm
- S7 https://help.solidworks.com/2024/English/SolidWorks/sldworks/c_Description_of_Sketch_Relations.htm
- S8 https://help.solidworks.com/2018/english/SolidWorks/sldworks/t_Using_Automatic_Relations.htm
- S9 https://help.solidworks.com/2016/english/solidworks/sldworks/hidd_make_driven.htm
- S10 https://help.solidworks.com/2026/english/SolidWorks/sldworks/t_Dimensioning_a_2D_Sketch.htm
- S11 https://help.solidworks.com/2017/English/SolidWorks/sldworks/c_Sketch_Modes.htm and https://help.solidworks.com/2018/english/SolidWorks/sldworks/t_Autotransitioning.htm
- S12 https://help.solidworks.com/2023/english/SWConnected/swdotworks/t_numeric_sketch_input.htm
- S13 https://help.solidworks.com/2022/English/SolidWorks/sldworks/c_Space_Handles.htm
- S14 https://help.solidworks.com/2025/english/SolidWorks/sldworks/c_box_selection.htm
- S15 https://help.solidworks.com/2025/english/solidworks/sldworks/HIDD_DVE_SK_EDIT_RELATIONS.htm

## Recommendation
1. **Navigation as data.** Model it as `NavPreset { bindings, wheel_in_toward_user, zoom_to_cursor, orbit_mode, pivot_policy, arrow_step_deg, transition_ms }`. Ship both presets and ask on first run. Default to SolidWorks bindings, because MMB-drag orbit needs no modifier.
2. **Shared keys.** Enable the non-conflicting Inventor keys in both presets: F2/F3/F4 hold, F5, F6, Home, End, Page Up.
3. **Orbit.** Free orbit by default with a turntable option. Pivot is the model point under the cursor at drag start, as Inventor 2024.1 does. Add SolidWorks' explicit MMB-click pivot.
4. **Wheel.** Keep the vendors' default of toward-user = zoom in, and offer "Reverse" in the first-run dialog.
5. **Up axis.** World Z-up, the robot convention, using the Z-up view table above. Keep a Y-up import option.
6. **Shell and colours.** Inventor-style shell (ribbon, browser with Origin folder, view cube, status-bar DOF counter, F8/F9 glyphs) with SolidWorks status colours. Inventor's own defaults are low-contrast.
7. **Sketch keys.** L, C, A, D, X, S, plus SolidWorks' A-key arc toggle alongside Inventor's drag gesture. Inventor constraint names plus Midpoint.
8. **Solver requirements.** Per-entity DOF, redundancy and conflict detection for the driven prompt, and minimal-displacement drag solving.

## Risks and unknowns
- **Verification is excerpt-level only.** `R` items need a hands-on check, notably:
  - Inventor: default colours, default Ctrl+MMB action, double-click MMB, marking-menu layout, grid defaults.
  - SolidWorks: default pivot, triad modifiers, gesture defaults.
- **Version drift.** Inventor navigation options changed in 2022 and 2024.1; SolidWorks status colours and names changed across releases.
- **Trademarks.** Imitate conventions, not names or icons (ViewCube, FeatureManager, PropertyManager, CommandManager).
- **Linux input conflicts.** Alt+MMB and Alt+drag are often taken by window managers, and holding F-keys while dragging needs raw key-state handling.
- **Solver fit.** Per-entity DOF colouring and conflict sets are not offered by every constraint solver.

## Surprises
- Both products zoom in when the wheel rolls toward the user. This is the opposite of AutoCAD, browsers and maps.
- MMB drag means pan in Inventor and rotate in SolidWorks, so one binding set cannot satisfy both. Inventor only made it remappable in 2022.
- Inventor has no Z-up option, only template / ViewCube redefinition. One forum post says the metric template is already Z-up while the inch template is Y-up (V2 I19, needs checking).
- SolidWorks' Z-up toggle (2020) changes views only, not plane names.
- SolidWorks' rotate-about-entity does not work while editing a sketch (V S4).
- SolidWorks colour semantics are not stable: over-defined red versus yellow "item conflicts", pink unsolved in older help, dangling brown then olive.
- Inventor's constrained colour is scheme-dependent, not a universal green-to-blue.
- Inventor documents Trim as alias TR and "-" as Horizontal constraint (V I2), and Relax Mode has its own key, F11.
- help.solidworks.com already serves 2027 pages, so fix the reference versions before final sign-off.
