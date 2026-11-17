//! User-interface text in Korean and English.
//!
//! Every message is a variant of [`Msg`], declared once together with both translations, so a missing
//! translation or a misspelt key cannot compile. Widgets and tests refer to messages by variant, never by
//! their text.

#![forbid(unsafe_code)]

use std::fmt::Display;

/// A user-interface language.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Lang {
    /// Korean (the default).
    #[default]
    Ko,
    /// English.
    En,
}

impl Lang {
    /// Every supported language.
    pub const ALL: [Self; 2] = [Self::Ko, Self::En];

    /// The language's own name, for the language menu.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::Ko => "한국어",
            Self::En => "English",
        }
    }

    /// The code stored in the settings file.
    pub fn code(self) -> &'static str {
        match self {
            Self::Ko => "ko",
            Self::En => "en",
        }
    }

    /// The language with the given settings code.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|lang| lang.code() == code)
    }
}

macro_rules! messages {
    ($( $(#[$doc:meta])* $key:ident => ($ko:literal, $en:literal), )*) => {
        /// A translatable message. `{0}`, `{1}`... in the text are filled by [`Msg::format`].
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Msg {
            $( $(#[$doc])* $key, )*
        }

        impl Msg {
            /// Every message, for tests that check the whole catalogue.
            pub const ALL: &'static [Self] = &[ $( Self::$key, )* ];

            /// The text in the given language.
            pub fn text(self, lang: Lang) -> &'static str {
                match (self, lang) {
                    $( (Self::$key, Lang::Ko) => $ko, (Self::$key, Lang::En) => $en, )*
                }
            }

            /// The variant name: a stable identifier that does not depend on the language.
            pub fn key(self) -> &'static str {
                match self {
                    $( Self::$key => stringify!($key), )*
                }
            }
        }
    };
}

messages! {
    /// Application name in the title bar.
    AppTitle => ("GraphCTRL", "GraphCTRL"),
    /// Menu: File.
    MenuFile => ("파일", "File"),
    /// Menu: Edit.
    MenuEdit => ("편집", "Edit"),
    /// Menu: View.
    MenuView => ("뷰", "View"),
    /// Menu: Tools.
    MenuTools => ("도구", "Tools"),
    /// Menu: Help.
    MenuHelp => ("도움말", "Help"),
    /// Command: New document.
    CmdNew => ("새로 만들기", "New"),
    /// Command: Open a document.
    CmdOpen => ("열기...", "Open..."),
    /// Command: Save the document.
    CmdSave => ("저장", "Save"),
    /// Command: Save under another name.
    CmdSaveAs => ("다른 이름으로 저장...", "Save As..."),
    /// Command: Quit the application.
    CmdQuit => ("끝내기", "Quit"),
    /// Command: Undo.
    CmdUndo => ("실행 취소", "Undo"),
    /// Command: Redo.
    CmdRedo => ("다시 실행", "Redo"),
    /// Command: Delete the selection.
    CmdDelete => ("삭제", "Delete"),
    /// Status bar while idle.
    StatusReady => ("준비", "Ready"),
    /// Status bar: how many dimensions a sketch still needs. `{0}` is the count.
    StatusDimensionsNeeded => ("{0}개 치수 필요", "{0} dimensions needed"),
    /// Status bar: the sketch is fully constrained.
    StatusFullyConstrained => ("완전 구속", "Fully constrained"),
    /// Ribbon tab: 3D model.
    TabModel => ("3D 모델", "3D Model"),
    /// Ribbon tab: sketch.
    TabSketch => ("스케치", "Sketch"),
    /// Ribbon tab: view.
    TabView => ("뷰", "View"),
    /// Ribbon group: start a sketch.
    GroupSketch => ("스케치", "Sketch"),
    /// Ribbon group: create geometry.
    GroupCreate => ("작성", "Create"),
    /// Ribbon group: constraints.
    GroupConstrain => ("구속", "Constrain"),
    /// Ribbon group: leave the sketch.
    GroupExit => ("종료", "Exit"),
    /// Ribbon group: standard views.
    GroupViews => ("표준 뷰", "Views"),
    /// Ribbon group: projection.
    GroupProjection => ("투영", "Projection"),
    /// Command: start a 2D sketch on a plane. `{0}` is the plane name.
    CmdSketchOn => ("{0} 스케치", "Sketch on {0}"),
    /// Command: finish the sketch.
    CmdFinishSketch => ("스케치 마무리", "Finish Sketch"),
    /// Tool: select.
    ToolSelect => ("선택", "Select"),
    /// Tool: line.
    ToolLine => ("선", "Line"),
    /// Tool: circle.
    ToolCircle => ("원", "Circle"),
    /// Tool: arc.
    ToolArc => ("호", "Arc"),
    /// Tool: rectangle.
    ToolRectangle => ("사각형", "Rectangle"),
    /// Tool: dimension.
    ToolDimension => ("치수", "Dimension"),
    /// Constraint: coincident.
    ConCoincident => ("일치", "Coincident"),
    /// Constraint: horizontal.
    ConHorizontal => ("수평", "Horizontal"),
    /// Constraint: vertical.
    ConVertical => ("수직", "Vertical"),
    /// Constraint: parallel.
    ConParallel => ("평행", "Parallel"),
    /// Constraint: perpendicular.
    ConPerpendicular => ("직각", "Perpendicular"),
    /// Constraint: collinear.
    ConCollinear => ("동일선상", "Collinear"),
    /// Constraint: tangent.
    ConTangent => ("접선", "Tangent"),
    /// Constraint: equal.
    ConEqual => ("동일", "Equal"),
    /// Constraint: concentric.
    ConConcentric => ("동심", "Concentric"),
    /// Constraint: midpoint.
    ConMidpoint => ("중점", "Midpoint"),
    /// Constraint: fix.
    ConFix => ("고정", "Fix"),
    /// Constraint: symmetric.
    ConSymmetric => ("대칭", "Symmetric"),
    /// View: front.
    ViewFront => ("정면", "Front"),
    /// View: top.
    ViewTop => ("평면", "Top"),
    /// View: right.
    ViewRight => ("우측", "Right"),
    /// View: home (isometric).
    ViewHome => ("홈", "Home"),
    /// View: zoom to fit everything.
    ViewFit => ("전체 보기", "Zoom All"),
    /// View: orthographic projection.
    ViewOrtho => ("직교", "Orthographic"),
    /// View: perspective projection.
    ViewPerspective => ("원근", "Perspective"),
    /// Panel title: model browser.
    PanelBrowser => ("모델", "Model"),
    /// Panel title: properties.
    PanelProperties => ("속성", "Properties"),
    /// Browser: origin folder.
    BrowserOrigin => ("원점", "Origin"),
    /// Browser: sketches folder.
    BrowserSketches => ("스케치", "Sketches"),
    /// Properties: nothing is selected.
    PropNothing => ("선택된 항목이 없습니다", "Nothing selected"),
    /// Properties: number of selected items. `{0}` is the count.
    PropSelected => ("{0}개 선택됨", "{0} selected"),
    /// Properties: a line and its length. `{0}` is the length.
    PropLine => ("선 · 길이 {0}", "Line · length {0}"),
    /// Properties: a circle and its radius. `{0}` is the radius.
    PropCircle => ("원 · 반지름 {0}", "Circle · radius {0}"),
    /// Properties: an arc and its radius. `{0}` is the radius.
    PropArc => ("호 · 반지름 {0}", "Arc · radius {0}"),
    /// Properties: a point and its coordinates. `{0}` and `{1}` are the coordinates.
    PropPoint => ("점 · ({0}, {1})", "Point · ({0}, {1})"),
    /// Properties: a dimension and its expression. `{0}` is the expression.
    PropDimension => ("치수 · {0}", "Dimension · {0}"),
    /// Properties: a geometric constraint.
    PropConstraint => ("구속조건", "Constraint"),
    /// Properties: label of the expression field of a dimension.
    PropExpression => ("수식", "Expression"),
    /// Prompt: select tool.
    PromptSelect => ("항목을 선택하거나 끌어서 이동", "Select items, or drag to move them"),
    /// Prompt: first point of a line.
    PromptLineStart => ("선의 시작점을 클릭", "Click the start of the line"),
    /// Prompt: next point of a line.
    PromptLineEnd => ("다음 점을 클릭 (Esc: 끝내기)", "Click the next point (Esc to finish)"),
    /// Prompt: centre of a circle.
    PromptCircleCentre => ("원의 중심을 클릭", "Click the centre of the circle"),
    /// Prompt: a point on the circle.
    PromptCircleRim => ("원 위의 한 점을 클릭", "Click a point on the circle"),
    /// Prompt: arc centre.
    PromptArcCentre => ("호의 중심을 클릭", "Click the centre of the arc"),
    /// Prompt: arc start.
    PromptArcStart => ("호의 시작점을 클릭", "Click where the arc starts"),
    /// Prompt: arc end.
    PromptArcEnd => ("호가 끝나는 곳까지 돌려서 클릭", "Sweep to where the arc ends and click"),
    /// Prompt: first corner of a rectangle.
    PromptRectFirst => ("첫 번째 구석을 클릭", "Click the first corner"),
    /// Prompt: opposite corner of a rectangle.
    PromptRectSecond => ("반대쪽 구석을 클릭", "Click the opposite corner"),
    /// Prompt: pick what to dimension.
    PromptDimPick => ("치수를 넣을 형상을 클릭", "Click the geometry to dimension"),
    /// Prompt: place the dimension.
    PromptDimPlace => ("치수를 놓을 위치를 클릭 (다른 형상을 클릭하면 둘 사이의 치수)", "Click where to place the dimension (or another item to dimension between them)"),
    /// Prompt: type the dimension value.
    PromptDimValue => ("값을 입력하고 Enter", "Type the value and press Enter"),
    /// Prompt: pick items for a constraint.
    PromptConstraint => ("구속할 형상을 차례로 클릭", "Click the items to constrain"),
    /// Status: the last edit was refused as redundant.
    ErrRedundant => ("이미 다른 구속으로 정해져 있습니다", "Already determined by other constraints"),
    /// Status: the last edit was refused as conflicting.
    ErrConflicting => ("다른 구속과 충돌합니다", "Conflicts with other constraints"),
    /// Status: the expression is not valid.
    ErrExpression => ("수식을 읽을 수 없습니다", "The expression is not valid"),
    /// Status: the sketch cannot be solved with this change.
    ErrUnsolved => ("이 변경으로는 스케치를 풀 수 없습니다", "The sketch cannot be solved with this change"),
    /// Status: any other refusal.
    ErrOther => ("적용할 수 없습니다", "This cannot be applied"),
    /// Edit refused: something else still uses the item.
    ErrInUse => ("아직 사용 중이라 삭제할 수 없습니다", "It is still in use and cannot be deleted"),
    /// Edit refused: a number is out of range.
    ErrBadValue => ("값이 범위를 벗어났습니다", "A value is out of range"),
    /// Question: add an over-constraining dimension as a reference dimension?
    AskReference => ("이 치수는 스케치를 과구속합니다. 참조 치수로 넣을까요?", "This dimension over-constrains the sketch. Add it as a reference dimension?"),
    /// Menu: Korean input test window.
    MenuImeTest => ("한글 입력 시험", "Korean input test"),
    /// Menu: language.
    MenuLanguage => ("언어", "Language"),
    /// Status: GPU in use.
    GpuOn => ("GPU 사용 중", "GPU in use"),
    /// Status: no GPU.
    GpuOff => ("GPU 없음", "No GPU"),
    /// Question before discarding unsaved changes.
    AskSaveChanges => ("변경 내용을 저장할까요?", "Save the changes?"),
    /// Button: do not save.
    BtnDiscard => ("저장 안 함", "Don't Save"),
    /// Status: a file operation failed. `{0}` is the reason.
    FileError => ("파일 오류: {0}", "File error: {0}"),
    /// Status: the document was saved. `{0}` is the file name.
    FileSaved => ("저장했습니다: {0}", "Saved: {0}"),
    /// Status: some sketches in the opened file do not satisfy their constraints. `{0}` is how many.
    FileNeedsRepair => ("구속을 만족하지 않는 스케치가 {0}개 있습니다", "{0} sketches do not satisfy their constraints"),
    /// Title of a document that has no file yet.
    Untitled => ("제목 없음", "Untitled"),
    /// Button: confirm.
    Ok => ("확인", "OK"),
    /// Button: cancel.
    Cancel => ("취소", "Cancel"),
    /// Ribbon tab: robot cell.
    TabRobot => ("로봇", "Robot"),
    /// Ribbon group: add things to the cell.
    GroupAdd => ("추가", "Add"),
    /// Ribbon group: waypoints and paths.
    GroupPath => ("경로", "Path"),
    /// Command: add a robot from the library. `{0}` is the model name.
    CmdAddRobot => ("{0} 로봇", "{0} Robot"),
    /// Command: add a box.
    CmdAddBox => ("상자", "Box"),
    /// Command: add a cylinder.
    CmdAddCylinder => ("원기둥", "Cylinder"),
    /// Command: add a sphere.
    CmdAddSphere => ("구", "Sphere"),
    /// Command: add a waypoint at the robot's current pose.
    CmdAddWaypoint => ("현재 자세", "Current Pose"),
    /// Command: add a waypoint at the selected sketch point.
    CmdWaypointAtPoint => ("스케치 점", "Sketch Point"),
    /// Command: create a path through the waypoints.
    CmdMakePath => ("경로 만들기", "Create Path"),
    /// Command: make a path that follows the lines of a sketch.
    CmdPathFromSketch => ("스케치 경로", "Path from Sketch"),
    /// Command: plan the path.
    CmdPlan => ("계획", "Plan"),
    /// Command: play the planned motion.
    CmdPlay => ("재생", "Play"),
    /// Command: stop playback.
    CmdStop => ("정지", "Stop"),
    /// Browser node: bodies.
    BrowserBodies => ("바디", "Bodies"),
    /// Browser node: robots.
    BrowserRobots => ("로봇", "Robots"),
    /// Browser node: waypoints.
    BrowserWaypoints => ("웨이포인트", "Waypoints"),
    /// Browser node: paths.
    BrowserPaths => ("경로", "Paths"),
    /// Panel section: jog the robot's joints.
    PanelJog => ("조그", "Jog"),
    /// Panel section: where the robot's base stands.
    PanelBase => ("로봇 베이스", "Robot Base"),
    /// Panel section: the tool centre point relative to the flange.
    PanelTcp => ("툴 중심점(TCP)", "Tool Centre Point (TCP)"),
    /// Panel section: placement and size of bodies.
    PanelBodies => ("바디 배치", "Body Placement"),
    /// Panel section: path planning.
    PanelPath => ("경로 계획", "Path Planning"),
    /// Label: position in millimetres.
    PropPosition => ("위치 (mm)", "Position (mm)"),
    /// Label: size in millimetres.
    PropSize => ("크기 (mm)", "Size (mm)"),
    /// Label: rotation as roll, pitch, yaw in degrees.
    PropRotation => ("회전 (도)", "Rotation (deg)"),
    /// Label: factor from file units to metres.
    PropScale => ("배율", "Scale"),
    /// Command: add a robot from a URDF file.
    CmdImportRobot => ("URDF 불러오기", "Import URDF"),
    /// Notice after importing a robot whose mesh files were not all found. `{0}` is how many.
    RobotMeshesMissing => ("메시 파일 {0}개를 찾지 못했습니다(해당 부분은 표시·충돌 검사에서 빠집니다)", "{0} mesh files were not found (those parts are neither drawn nor checked)"),
    /// Command: add a body from a mesh file.
    CmdImportMesh => ("메시 불러오기", "Import Mesh"),
    /// Label: planner choice.
    PropPlanner => ("플래너", "Planner"),
    /// Label: random seed.
    PropSeed => ("시드", "Seed"),
    /// Label: playback time.
    PropTime => ("시간 (s)", "Time (s)"),
    /// Plan status: never planned.
    PlanNotPlanned => ("계획 안 됨", "Not planned"),
    /// View cube face: back.
    ViewBack => ("배면", "Back"),
    /// View cube face: left.
    ViewLeft => ("좌측", "Left"),
    /// View cube face: bottom.
    ViewBottom => ("저면", "Bottom"),
    /// Command: run every planner on the path and tabulate the results.
    CmdCompare => ("알고리즘 비교", "Compare Planners"),
    /// Comparison table column: successful runs out of all runs.
    ColSuccess => ("성공", "Success"),
    /// Comparison table column: median estimated duration.
    ColDuration => ("시간 s", "Time s"),
    /// Comparison table column: median state checks.
    ColChecks => ("검사 수", "Checks"),
    /// Comparison note: computed for an earlier state of the cell.
    CompareStale => ("이전 상태 기준 결과", "Computed for an earlier state"),
    /// Button beside a browser item: delete it. `{0}` is the item's name.
    CmdDeleteItem => ("{0} 삭제", "Delete {0}"),
    /// Browser node: named parameters.
    BrowserParams => ("파라미터", "Parameters"),
    /// Parameter editor: name field hint.
    ParamName => ("이름", "name"),
    /// Parameter editor: expression field hint.
    ParamValue => ("값 (예: 20 mm)", "value (e.g. 20 mm)"),
    /// Parameter editor: add a new parameter.
    ParamAdd => ("추가", "Add"),
    /// Parameter editor: change the parameter with this name.
    ParamApply => ("적용", "Apply"),
    /// Command: start a 3D sketch.
    CmdSketch3d => ("3D 스케치", "3D Sketch"),
    /// 3D constraint: along X.
    ConAlongX => ("X 방향", "Along X"),
    /// 3D constraint: along Y.
    ConAlongY => ("Y 방향", "Along Y"),
    /// 3D constraint: along Z.
    ConAlongZ => ("Z 방향", "Along Z"),
    /// 3D constraint: on the XY plane.
    ConOnXY => ("XY 평면 위", "On XY"),
    /// 3D constraint: on the YZ plane.
    ConOnYZ => ("YZ 평면 위", "On YZ"),
    /// 3D constraint: on the ZX plane.
    ConOnZX => ("ZX 평면 위", "On ZX"),
    /// Status bar: the working plane of 3D sketching. `{0}` is its name.
    StatusPlane => ("작업 평면 {0} (Tab으로 전환)", "Working plane {0} (Tab switches)"),
    /// Checkbox beside a waypoint of a path: go there in a straight line of the tool tip. `{0}` is the
    /// waypoint's name.
    PropLinear => ("{0}까지 직선(LIN)", "Straight line (LIN) to {0}"),
    /// The robot as shown is in collision. `{0}` and `{1}` are the two parts.
    StateCollision => ("충돌: {0} / {1}", "Collision: {0} / {1}"),
    /// The robot as shown is closer to a body than allowed. `{0}`, `{1}`: the parts; `{2}`: the gap in mm.
    StateTooClose => ("너무 가까움: {0} / {1} (간격 {2} mm)", "Too close: {0} / {1} (gap {2} mm)"),
    /// A joint of the robot as shown is beyond its limit. `{0}` is the joint.
    StateJointLimit => ("관절 한계 초과: {0}", "Joint beyond its limit: {0}"),
    /// The state of the robot could not be checked.
    StateUnknown => ("이 자세는 검사할 수 없습니다", "This pose could not be checked"),
    /// Command: make the selected, fully constrained sketch points (origin, X axis point, XY plane point,
    /// in the order picked) a work frame.
    CmdWorkFrame => ("좌표계", "Work Frame"),
    /// Command: add a waypoint at the robot's current pose, stored relative to a work frame.
    CmdWaypointInFrame => ("좌표계 기준", "In Frame"),
    /// Browser node: work frames.
    BrowserFrames => ("좌표계", "Work Frames"),
    /// Panel section: the work frames and the waypoints defined in them.
    PanelFrames => ("좌표계 기준 위치", "Positions in Work Frames"),
    /// Label of a waypoint defined in a work frame. `{0}`: waypoint, `{1}`: frame.
    PropInFrame => ("{0} (기준: {1})", "{0} (in {1})"),
    /// Label: the frame waypoints are taught in. `{0}` is its name.
    PropTeachFrame => ("기준 좌표계: {0}", "Teach in: {0}"),
    /// Edit refused: the axis points of a work frame coincide with its origin or lie on one line.
    ErrDegenerateFrame => ("축을 정할 점이 원점과 겹치거나 원점과 한 직선 위에 있습니다", "The axis points coincide with the origin or lie on one line with it"),
    /// Work frame: the point the X axis points to.
    PropXPoint => ("X축 방향 점", "X axis towards"),
    /// Work frame: a point on the positive-Y side of the XY plane.
    PropXyPoint => ("XY 평면 위 점(+Y 쪽)", "Point in XY (+Y side)"),
    /// Work frame: no point chosen; the sketch's own axes are used.
    PropSketchAxes => ("스케치 축", "Sketch axes"),
    /// Work frame: its pose in the world, as X, Y, Z (mm) and roll, pitch, yaw (deg).
    PropPoseWorld => ("월드 기준", "In the world"),
    /// Work frame: its pose in the robot's base frame.
    PropPoseBase => ("로봇 베이스 기준", "In the robot base"),
    /// Rotation as a rotation vector, the form UR controllers show.
    PropRotvec => ("회전 벡터(UR)", "Rotation vector (UR)"),
    /// Edit refused: a work frame's origin must be fully constrained.
    ErrUnderconstrained => ("좌표계를 정할 점이 완전히 구속되지 않았습니다(치수나 고정 구속을 추가하세요)", "A point of the frame is not fully constrained (add dimensions or fix it)"),

    PlanPlanning => ("계획 중...", "Planning..."),
    /// Plan status: up to date.
    PlanReady => ("계획됨", "Planned"),
    /// Plan status: out of date.
    PlanStale => ("오래됨(다시 계획 필요)", "Out of date (plan again)"),
    /// Plan status: failed.
    PlanFailed => ("실패", "Failed"),
    /// Plan metric: estimated duration. `{0}` is seconds.
    PlanDuration => ("예상 시간 {0} s", "Estimated time {0} s"),
    /// Plan metric: smallest clearance. `{0}` is millimetres.
    PlanClearance => ("최소 여유 {0} mm", "Minimum clearance {0} mm"),
    /// Plan metric: planner effort. `{0}` is a count.
    PlanChecks => ("상태 검사 {0}회", "{0} state checks"),
    /// Plan metric: the move was possible in a straight joint-space line.
    PlanStraight => ("직선 이동(플래너 불필요)", "Straight move (no planner needed)"),
}

impl Msg {
    /// The text with `{0}`, `{1}`... replaced by the arguments. Placeholders without an argument stay as
    /// they are, so a mistake is visible instead of silent.
    pub fn format(self, lang: Lang, args: &[&dyn Display]) -> String {
        let mut out = self.text(lang).to_owned();
        for (index, arg) in args.iter().enumerate() {
            out = out.replace(&format!("{{{index}}}"), &arg.to_string());
        }
        out
    }
}

/// Shorthand for [`Msg::text`] and [`Msg::format`]: `tr!(lang, StatusReady)`, `tr!(lang, Key, a, b)`.
#[macro_export]
macro_rules! tr {
    ($lang:expr, $key:ident) => {
        $crate::Msg::$key.text($lang)
    };
    ($lang:expr, $key:ident, $($arg:expr),+ $(,)?) => {
        $crate::Msg::$key.format($lang, &[$(&$arg),+])
    };
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The placeholders (`{0}`, `{1}`...) that occur in a text.
    fn placeholders(text: &str) -> BTreeSet<usize> {
        (0..10).filter(|index| text.contains(&format!("{{{index}}}"))).collect()
    }

    #[test]
    fn every_message_has_text_in_every_language() {
        for message in Msg::ALL {
            for lang in Lang::ALL {
                assert!(!message.text(lang).trim().is_empty(), "{} is empty in {lang:?}", message.key());
            }
        }
    }

    #[test]
    fn translations_use_the_same_placeholders() {
        for message in Msg::ALL {
            assert_eq!(
                placeholders(message.text(Lang::Ko)),
                placeholders(message.text(Lang::En)),
                "{}",
                message.key()
            );
        }
    }

    #[test]
    fn keys_are_unique() {
        let keys: BTreeSet<&str> = Msg::ALL.iter().map(|m| m.key()).collect();
        assert_eq!(keys.len(), Msg::ALL.len());
    }

    #[test]
    fn formatting_fills_placeholders() {
        assert_eq!(tr!(Lang::Ko, StatusDimensionsNeeded, 3), "3개 치수 필요");
        assert_eq!(tr!(Lang::En, StatusDimensionsNeeded, 3), "3 dimensions needed");
        assert_eq!(tr!(Lang::Ko, StatusReady), "준비");
        assert_eq!(Msg::StatusDimensionsNeeded.format(Lang::En, &[]), "{0} dimensions needed");
    }

    #[test]
    fn language_codes_round_trip() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_code(lang.code()), Some(lang));
        }
        assert_eq!(Lang::from_code("fr"), None);
        assert_eq!(Lang::default(), Lang::Ko);
    }
}
