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
    GroupNavigate => ("탐색", "Navigate"),
    GroupWindows => ("창", "Windows"),
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
    ToolArc => ("중심점 호", "Center Point Arc"),
    /// Tool: rectangle.
    ToolRectangle => ("2점 직사각형", "Two Point Rectangle"),
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
    ConSmooth => ("부드럽게(G2)", "Smooth (G2)"),
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
    ObjVisTitle => ("개체 가시성", "Object Visibility"),
    ObjVisOriginPlanes => ("원점 평면", "Origin Planes"),
    ObjVisOriginAxes => ("원점 축", "Origin Axes"),
    ObjVisOriginPoints => ("원점 점", "Origin Points"),
    ObjVisWorkPlanes => ("작업 평면", "Work Planes"),
    ObjVisWorkAxes => ("작업 축", "Work Axes"),
    ObjVisWorkPoints => ("작업 점", "Work Points"),
    ObjVisSketches => ("스케치", "Sketches"),
    ObjVisSketches3d => ("3D 스케치", "3D Sketches"),
    ObjVisConstraints => ("구속조건 기호", "Constraint Symbols"),
    UiTitle => ("사용자 인터페이스", "User Interface"),
    UiViewCube => ("뷰큐브", "ViewCube"),
    UiNavBar => ("탐색 막대", "Navigation Bar"),
    UiBrowser => ("브라우저", "Browser"),
    UiProperties => ("속성", "Properties"),
    UiStatusBar => ("상태 표시줄", "Status Bar"),
    CleanScreen => ("전체 화면(Ctrl+0)", "Clean Screen (Ctrl+0)"),
    GroundPlane => ("바닥 평면", "Ground Plane"),
    VisualStyleTitle => ("시각 스타일", "Visual Style"),
    VisualStyleShaded => ("음영 처리", "Shaded"),
    VisualStyleShadedEdges => ("모서리가 있는 음영 처리", "Shaded with Edges"),
    VisualStyleShadedHidden => ("숨은 모서리가 있는 음영 처리", "Shaded with Hidden Edges"),
    VisualStyleWireframeHidden => ("숨은 모서리가 있는 와이어프레임", "Wireframe with Hidden Edges"),
    VisualStyleWireframeVisible => ("보이는 모서리만 있는 와이어프레임", "Wireframe with visible Edges Only"),
    VisualStyleWireframe => ("와이어프레임", "Wireframe"),
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
    /// Menu: recently used projects.
    MenuRecent => ("최근 프로젝트", "Recent Projects"),
    /// Question at start: unsaved work from a session that did not end normally was found.
    AskRecover => ("저장하지 않은 작업이 자동 저장되어 있습니다(프로그램이 정상 종료되지 않음). 복구할까요?", "Unsaved work was saved automatically (the program did not close normally). Recover it?"),
    /// Button: recover the automatic save.
    BtnRecover => ("복구", "Recover"),
    /// Notice after recovering. `{0}` is the project, or empty for a new one.
    FileWarnings => ("열었지만 쓸 수 없는 데이터 {0}건을 버렸습니다: {1}", "Opened, but {0} unusable item(s) were dropped: {1}"),
    FileRecovered => ("복구했습니다. 저장해야 남습니다. {0}", "Recovered. Save it to keep it. {0}"),
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
    MenuHelp => ("도움말", "Help"),
    MenuLicences => ("라이선스", "Licences"),
    PropTaughtJoints => ("관절값 (현재 자세로 저장된 웨이포인트)", "Joint values (taught waypoint)"),
    CmdTeachAgain => ("지금 로봇 자세로 다시 저장", "Store the robot's current pose"),
    PropOnSketchPoint => ("스케치 점에 붙어 있음({0}): 스케치를 고치면 따라 움직입니다", "On a point of {0}: it moves when the sketch changes"),
    PropTcpLimit => ("직선 이동 TCP 속도 제한", "Limit tool speed on straight moves"),
    PlanPeakTcp => ("최대 TCP 속도: {0} mm/s", "Highest tool speed: {0} mm/s"),
    CmdRobotLibrary => ("로봇 추가", "Add robot"),
    RefWorld => ("월드", "World"),
    RefBase => ("로봇 베이스", "Robot base"),
    RefJoints => ("관절값", "Joint values"),
    RefSketchPoint => ("스케치 점", "Sketch point"),
    PropReference => ("기준 좌표계", "Measured from"),
    PropSeenFrom => ("다른 좌표계에서 본 값", "Seen from other frames"),
    PropMatrix => ("변환 행렬 (이동: mm)", "Transform matrix (translation in mm)"),
    CmdCopy => ("복사", "Copy"),
    CmdUseAsA => ("A로", "As A"),
    MenuPoseCalc => ("포즈 변환 계산기", "Pose calculator"),
    PropRotationInput => ("회전 입력", "Rotation as"),
    InputRpy => ("RPY (°)", "RPY (°)"),
    InputRotvec => ("회전벡터 (rad)", "Rotation vector (rad)"),
    InputQuaternion => ("쿼터니언", "Quaternion"),
    CalcPoseA => ("포즈 A", "Pose A"),
    CalcPoseB => ("포즈 B", "Pose B"),
    CmdFromWaypoint => ("선택한 웨이포인트 → A", "Selected waypoint → A"),
    CalcFrom => ("A의 기준", "A is in"),
    CalcTo => ("변환할 기준", "Show A in"),
    CalcExpressed => ("A: {0} 기준 → {1} 기준", "A: from {0} to {1}"),
    CalcCompose => ("A · B", "A · B"),
    CalcInverse => ("A^-1", "A^-1"),
    CalcRelative => ("A^-1 · B (A에서 본 B)", "A^-1 · B (B seen from A)"),
    PropSteps => ("웨이포인트 순서", "Waypoints in order"),
    PropResult => ("결과", "Result"),
    IkAnalytic => ("IK: 해석적 ({0}) · 한계 안의 자세 {1}개", "IK: closed form ({0}) · {1} configurations within limits"),
    IkNumeric => ("IK: 수치 계산 · 찾은 자세 {0}개", "IK: numerical · {0} configurations found"),
    MenuAbout => ("정보", "About"),
    AboutVersion => ("버전 {0}", "Version {0}"),
    AboutSettings => ("설정 파일: {0}", "Settings file: {0}"),
    AboutRecovery => ("자동 저장(복구) 폴더: {0}", "Autosave (recovery) folder: {0}"),
    AboutNoSettings => ("설정 폴더를 찾지 못해 설정과 자동 저장을 쓰지 않습니다.", "No settings folder was found: settings and autosave are off."),
    ExampleNeedsRobot => ("예제에 필요한 로봇({0})이 로봇 라이브러리에 없습니다", "The example needs the {0} from the robot library"),
    LibraryEmpty => ("로봇 라이브러리(robots 폴더)를 찾지 못했습니다", "No robot library (robots folder) was found"),
    CmdExample => ("예제 열기 (UR5e 셀)", "Open example (UR5e cell)"),
    ExampleFailed => ("예제를 열 수 없습니다: {0}", "The example could not be opened: {0}"),
    PanelTool => ("툴", "Tool"),
    PropToolShape => ("툴 형상", "Tool shape"),
    ToolNone => ("없음", "None"),
    ToolCylinder => ("원기둥", "Cylinder"),
    ToolBox => ("상자", "Box"),
    ToolSphere => ("구", "Sphere"),
    ToolMesh => ("메시", "Mesh"),
    ToolFromFile => ("메시 파일에서 불러오기…", "Load from a mesh file…"),
    ToolTcpAtEnd => ("TCP를 툴 끝으로", "TCP at the tool's end"),
    StepUp => ("↑", "↑"),
    StepDown => ("↓", "↓"),
    StepRemove => ("빼기", "Remove"),
    StepAdd => ("웨이포인트 추가", "Add waypoint"),
    ReachOk => ("로봇이 도달합니다(반투명 로봇이 그 자세)", "The robot reaches it (the see-through arm shows how)"),
    ReachOut => ("로봇이 닿지 않습니다", "The robot cannot reach it"),
    ReachProblem => ("도달하지만 문제가 있습니다: {0}", "Reached, but: {0}"),
    /// Label: the frame waypoints are taught in. `{0}` is its name.
    PropTeachFrame => ("새 웨이포인트 기준: {0}", "New waypoints in: {0}"),
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
    /// Checkbox beside a body or robot in the browser: show it. `{0}` is its name.
    ShowItem => ("{0} 보이기", "Show {0}"),
    /// Slider: how see-through a body or robot is, in percent.
    PropTransparency => ("투명도", "Transparency"),
    /// Context menu: hide the body or robot.
    CmdHide => ("숨기기", "Hide"),
    /// Context menu: show the body or robot.
    CmdShow => ("보이기", "Show"),
    /// Properties panel heading when nothing is selected: everything of the cell is listed.
    PropCellOverview => ("셀 전체 (뷰포트나 브라우저에서 항목을 클릭하면 그 항목만 표시)", "Whole cell (click an item in the view or browser to show only it)"),

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
    // --- frontend ---
    /// Browser: the links of a robot.
    BrowserLinks => ("링크", "Links"),
    /// Ribbon: the Joint command; the dialog's title.
    JointTitle => ("조인트", "Joint"),
    /// Joint dialog: the type chosen from the two origins.
    JointAutomatic => ("자동", "Automatic"),
    /// Joint dialog: a joint that removes all freedom.
    JointRigid => ("강체", "Rigid"),
    /// Joint dialog: a joint with one rotation.
    JointRotational => ("회전", "Rotational"),
    /// Joint dialog: the origin on the component that moves.
    JointFirst => ("첫 번째 원점", "First origin"),
    /// Joint dialog: the origin on the component that stays.
    JointSecond => ("두 번째 원점", "Second origin"),
    /// Joint dialog: reverse the direction of the component.
    JointFlip => ("구성요소 뒤집기", "Flip component"),
    /// Marking menu of the assembly environment: place a component.
    MarkPlace => ("부품 배치", "Place component"),
    /// Marking menu of the assembly environment: move a component.
    MarkMove => ("컴포넌트 이동", "Move component"),
    /// Marking menu of the assembly environment: rotate a component.
    MarkRotate => ("컴포넌트 회전", "Rotate component"),
    /// View tab group of appearance commands (section views).
    GroupAppearance => ("외형", "Appearance"),
    /// Section view types and mini-toolbar.
    SectionHalf => ("1/2 단면", "Half section"),
    /// Section view type.
    SectionQuarter => ("1/4 단면", "Quarter section"),
    /// Section view type.
    SectionThreeQuarter => ("3/4 단면", "Three-quarter section"),
    /// Section view: remove it.
    SectionDelete => ("단면 삭제", "Delete section view"),
    /// Section mini-toolbar: the position of the cutting plane.
    SectionDistance => ("위치", "Position"),
    /// Section mini-toolbar: flip the cut direction.
    SectionFlip => ("뒤집기", "Flip"),
    /// Section mini-toolbar: go on to the second plane.
    SectionContinue => ("계속", "Continue"),
    /// Section mini-toolbar: apply.
    SectionOk => ("확인", "OK"),
    /// Section mini-toolbar: cancel.
    SectionCancel => ("취소", "Cancel"),
    /// Ribbon: the Constrain command; the dialog's title.
    ConstrainTitle => ("구속", "Constrain"),
    /// Constrain dialog: the Mate type.
    ConstrainMate => ("맞춤", "Mate"),
    /// Constrain dialog: the Flush type.
    ConstrainFlush => ("플러시", "Flush"),
    /// Constrain dialog: the Insert type.
    ConstrainInsert => ("삽입", "Insert"),
    /// Constrain dialog: Predict Offset and Orientation.
    ConstrainPredict => ("오프셋 및 방향 예측", "Predict offset and orientation"),
    /// Constrain dialog: the first selection.
    ConstrainMore => ("더 보기", "More"),
    ConstrainName => ("이름", "Name"),
    ConstrainMax => ("최대", "Maximum"),
    ConstrainMin => ("최소", "Minimum"),
    ConstrainResting => ("오프셋을 휴지 위치로", "Use Offset As Resting Position"),
    ConstrainFirst => ("첫 번째 선택", "First selection"),
    /// Constrain dialog: the second selection.
    ConstrainSecond => ("두 번째 선택", "Second selection"),
    /// Constrain dialog: nothing picked yet.
    ConstrainPick => ("뷰에서 선택", "pick in the view"),
    /// Constrain dialog: axis direction solutions.
    ConstrainOpposed => ("반대", "Opposed"),
    /// Constrain dialog: axis direction solutions.
    ConstrainAligned => ("정렬", "Aligned"),
    /// Constrain dialog: axis direction solutions.
    ConstrainUndirected => ("방향 없음", "Undirected"),
    /// Constrain dialog: the offset field.
    ConstrainOffset => ("오프셋", "Offset"),
    /// Constrain dialog buttons.
    ConstrainOk => ("확인", "OK"),
    /// Constrain dialog buttons.
    ConstrainApply => ("적용", "Apply"),
    /// Constrain dialog buttons.
    ConstrainCancel => ("취소", "Cancel"),
    /// Kinds of selected geometry.
    ConstrainPlane => ("평면", "plane"),
    /// Kinds of selected geometry.
    ConstrainAxis => ("축", "axis"),
    /// Kinds of selected geometry.
    ConstrainPoint => ("점", "point"),
    /// Kinds of selected geometry.
    ConstrainCircle => ("원", "circle"),
    /// Constrain: a joint origin (a frame: point and axes).
    ConstrainFrame => ("조인트 원점", "joint origin"),
    /// Constrain: selected geometry of a kind without its own word yet.
    ConstrainGeometry => ("형상", "geometry"),
    /// Status bar: the constraint cannot be solved with the others.
    ErrUnsolvable => ("구속을 풀 수 없습니다", "The constraints cannot be solved"),
    /// Status bar: the number of occurrences (components) in the document.
    StatusOccurrences => ("구성요소 {0}", "Occurrences {0}"),
    /// Status bar: the number of documents open.
    StatusDocuments => ("문서 {0}", "Documents {0}"),
    /// Navigation bar tool names.
    NavZoomAll => ("모두 확대", "Zoom All"),
    /// Navigation bar tool.
    NavPan => ("초점 이동", "Pan"),
    /// Navigation bar tool.
    NavZoom => ("확대/축소", "Zoom"),
    /// Navigation bar tool.
    NavOrbit => ("궤도", "Orbit"),
    /// Navigation bar tool.
    NavLookAt => ("면 보기", "Look At"),
    /// The ViewCube's home button.
    NavHome => ("홈 뷰", "Home view"),
    /// Browser: the root node's name when the document has no file yet.
    BrowserDocument => ("Cell1", "Cell1"),
    /// Browser: the folder of relationships (constraints and joints).
    BrowserRelationships => ("관계", "Relationships"),
    /// Browser: the folder of representations (views, positions).
    BrowserRepresentations => ("표현", "Representations"),
    /// Context menu of an origin plane: start a sketch on it.
    CmdSketchHere => ("스케치 만들기", "Create sketch"),
    /// Origin folder items of a component.
    OriginYz => ("YZ 평면", "YZ plane"),
    /// Origin folder item.
    OriginXz => ("XZ 평면", "XZ plane"),
    /// Origin folder item.
    OriginXy => ("XY 평면", "XY plane"),
    /// Origin folder item.
    OriginAxisX => ("X축", "X axis"),
    /// Origin folder item.
    OriginAxisY => ("Y축", "Y axis"),
    /// Origin folder item.
    OriginAxisZ => ("Z축", "Z axis"),
    /// Origin folder item.
    OriginPoint => ("중심점", "Center point"),
    /// Origin folder menu: show all seven elements.
    CmdShowOrigin => ("원점 표시", "Show origin"),
    /// Origin folder menu: hide all seven elements.
    CmdHideOrigin => ("원점 숨기기", "Hide origin"),
    /// Browser group of imported equipment.
    BrowserAssemblies => ("설비", "Equipment"),
    /// Ribbon tab for measuring and analysing the cell.
    TabInspect => ("검사", "Inspect"),
    /// Ribbon tab: Inventor's Manage tab.
    TabManage => ("관리", "Manage"),
    /// Manage tab group of the parameters.
    GroupParameters => ("매개변수", "Parameters"),
    /// Inspect tab group: measuring.
    GroupMeasure => ("측정", "Measure"),
    /// Inspect tab group: analyses.
    GroupAnalyze => ("분석", "Analyze"),
    /// Robot tab group: relationships and motion of components.
    GroupRelationships => ("관계", "Relationships"),
    /// Heads-up display box names.
    HudLength => ("길이", "Length"),
    /// Heads-up display box: direction.
    HudAngle => ("각도", "Angle"),
    /// Heads-up display box.
    HudRadius => ("반지름", "Radius"),
    /// Heads-up display box.
    HudDiameter => ("지름", "Diameter"),
    /// Heads-up display box.
    HudWidth => ("너비", "Width"),
    /// Heads-up display box.
    HudHeight => ("높이", "Height"),
    /// Status bar button: cut away what hides the sketch plane (F7).
    StatusSlice => ("그래픽 자르기 (F7)", "Slice Graphics (F7)"),
    /// Sketch context menu: end the command in progress.
    CmdDone => ("완료", "Done"),
    /// Browser menu: rename in place.
    CmdRename => ("이름 바꾸기", "Rename"),
    /// Browser menu: suppress an item (kept, not computed).
    CmdSuppress => ("억제", "Suppress"),
    CmdEnabled => ("활성", "Enabled"),
    BrowserMenu => ("브라우저 메뉴", "Browser menu"),
    BrowserHideWork => ("작업 피처 숨기기", "Hide Work Features"),
    BrowserHideConstrained => ("완전 구속 숨기기", "Hide Fully Constrained"),
    BrowserHideWarnings => ("경고 숨기기", "Hide Warnings"),
    BrowserShowState => ("구속 상태 표시", "Show Constraint State"),
    BrowserEditValues => ("브라우저에서 값 편집", "Edit Values in Browser"),
    BrowserRelationValue => ("관계 값", "Relationship value"),
    BrowserFind => ("찾기", "Find"),
    BrowserFindHint => ("이름으로 찾기", "Find by name"),
    /// Browser menu: bring a suppressed item back.
    CmdUnsuppress => ("억제 해제", "Unsuppress"),
    /// Status bar: the name is taken by another item of the same kind.
    ErrNameTaken => ("같은 이름이 이미 있습니다", "That name is already taken"),
    /// View tab: the dark colour scheme.
    ViewDarkTheme => ("어두운 테마", "Dark theme"),
    /// Ribbon group of the View tab that holds the panel options.
    GroupPanels => ("패널", "Panels"),
    /// Folds a floating side panel away. `{0}` is the panel's name.
    PanelFold => ("{0} 접기", "Fold {0}"),
    /// Brings a folded side panel back. `{0}` is the panel's name.
    PanelUnfold => ("{0} 펼치기", "Unfold {0}"),
    /// View tab: whether the side panels let the 3D scene show through.
    ViewPanelsSeeThrough => ("패널 반투명", "See-through panels"),
    /// Help menu: the log window.
    MenuLog => ("로그", "Log"),
    /// Constraint error dialog: title.
    ConflictTitle => ("구속 오류", "Constraint error"),
    /// Constraint error dialog: what was refused. `{0}` lists the relationships that are not satisfied.
    ConflictMessage => ("{0}을(를) 함께 만족할 수 없습니다.", "{0} cannot be satisfied together."),
    /// Constraint error dialog: heading of the list of relationships in the way.
    ConflictCulprits => ("걸림이 되는 관계 (하나만 억제하면 풀림)", "Relationships in the way (suppressing one solves it)"),
    /// Constraint error dialog: no single relationship is the cause.
    ConflictNoSingle => ("한 관계만으로는 풀리지 않습니다. 새 관계를 바꾸세요.", "No single relationship is the cause. Change the new relationship."),
    /// Constraint error dialog: delete the relationship in the way.
    ConflictDelete => ("삭제", "Delete"),
    /// Pattern dialog: the Assemble tab command and the dialog's title.
    PatternTitle => ("패턴", "Pattern"),
    /// Pattern dialog: tab for columns and rows.
    PatternRectangular => ("사각형", "Rectangular"),
    /// Pattern dialog: tab for copies round an axis.
    PatternCircular => ("원형", "Circular"),
    /// Pattern dialog: the components that are copied.
    PatternComponents => ("패턴화할 컴포넌트", "Components to pattern"),
    /// Pattern dialog: the column direction.
    PatternColumn => ("열", "Column"),
    /// Pattern dialog: the row direction.
    PatternRow => ("행", "Row"),
    /// Pattern dialog: the rotation axis.
    PatternAxis => ("회전 축", "Rotation axis"),
    /// Pattern dialog: reverse the direction.
    PatternFlip => ("뒤집기", "Flip"),
    /// Pattern dialog: spread both ways from the parent.
    PatternMidplane => ("중간 평면", "Midplane"),
    /// Pattern dialog: number of occurrences.
    PatternCount => ("개수", "Count"),
    /// Pattern dialog: distance between occurrences in millimetres.
    PatternSpacing => ("간격 (mm)", "Spacing (mm)"),
    /// Pattern dialog: angle in degrees.
    PatternAngle => ("각도 (°)", "Angle (°)"),
    /// Pattern dialog: copies turn as they go round.
    PatternRotational => ("회전", "Rotational"),
    /// Pattern dialog: copies keep the parent's orientation.
    PatternFixed => ("고정", "Fixed"),
    /// Pattern dialog: the angle is between occurrences.
    PatternIncremental => ("증분", "Incremental"),
    /// Pattern dialog: the angle is the total the pattern covers.
    PatternFitted => ("맞춤", "Fitted"),
    /// Browser: the folder of component patterns.
    BrowserPatterns => ("패턴", "Patterns"),
    /// Ribbon: Inventor's Assemble tab.
    TabAssemble => ("조립", "Assemble"),
    /// Assemble tab: group of the place command.
    GroupComponent => ("컴포넌트", "Component"),
    /// Assemble tab: places a component (the mesh and assembly import).
    CmdPlace => ("배치", "Place"),
    /// Assemble tab: group of free move and free rotate.
    GroupPosition => ("위치", "Position"),
    /// View tab: group of what is drawn on top of the model.
    GroupVisibility => ("가시성", "Visibility"),
    /// View tab, Visibility: shows the degrees-of-freedom symbols of the components.
    CmdDof => ("자유도", "Degrees of Freedom"),
    /// Browser, right-click a component: it takes part in the contact solver's analysis.
    CmdContactSet => ("접촉 세트", "Contact Set"),
    /// Inspect tab: activates the contact solver (a menu of the modes).
    CmdContactSolver => ("접촉 솔버", "Contact Solver"),
    /// Contact solver mode: off.
    ContactOff => ("접촉 솔버 끔", "Contact Solver Off"),
    /// Contact solver mode: only the components of the contact set.
    ContactSetOnly => ("접촉 세트만", "Contact Set Only"),
    /// Contact solver mode: every component of the assembly.
    ContactAll => ("모든 컴포넌트", "All Components"),
    /// Assemble tab: mirrors components across a plane; the dialog's tab.
    ReplicateMirror => ("미러", "Mirror"),
    /// Assemble tab: copies components; the dialog's tab.
    ReplicateCopy => ("복사", "Copy"),
    /// Assemble tab: replaces a component with another shape; the dialog's tab.
    ReplicateReplace => ("교체", "Replace"),
    /// Mirror/copy/replace dialog: the components the command works on.
    ReplicateComponents => ("컴포넌트", "Components"),
    /// Mirror dialog: the mirror plane.
    ReplicatePlane => ("대칭면", "Mirror plane"),
    /// Copy dialog: the offset of the copy. Followed by the axis and the unit.
    ReplicateOffset => ("이동", "Offset"),
    /// Replace dialog: a box.
    ReplicateBox => ("박스", "Box"),
    /// Replace dialog: a cylinder.
    ReplicateCylinder => ("원기둥", "Cylinder"),
    /// Replace dialog: a sphere.
    ReplicateSphere => ("구", "Sphere"),
    /// Replace dialog: every component of the same source.
    ReplicateAll => ("모두 교체", "Replace All"),
    /// Mirror and copy dialog: make the relationships between the components for the copies too.
    ReplicateRelationships => ("관계 복사", "Copy relationships"),
    /// Mirror and copy dialog: ground the new components.
    ReplicateGround => ("새 컴포넌트 고정", "Ground new components"),
    /// Mirror, copy and replace dialog: done.
    ReplicateDone => ("완료했습니다.", "Done."),
    /// Mirror, copy and replace dialog: relationships that could not be kept. `{0}` lists them.
    ReplicateSuppressed => ("억제된 관계(다시 적용하세요): {0}", "Suppressed relationships (apply again): {0}"),
    /// Browser, Representations folder: the view representations.
    RepViews => ("뷰 표현", "View representation"),
    /// Browser, Representations folder: the positional representations.
    RepPositions => ("위치 표현", "Positional representation"),
    /// Browser, Representations folder: creates one. `{0}` is the kind.
    RepNew => ("새 {0}", "New {0}"),
    /// Sketch ribbon, split buttons: the accessible name of the arrow. `{0}` is the group.
    SplitMore => ("{0} 더 보기", "More {0}"),
    /// Sketch ribbon, split buttons: the arc group.
    SplitArc => ("호", "Arc"),
    /// Sketch ribbon, split buttons: the rectangle group.
    SplitRectangle => ("직사각형", "Rectangle"),
    /// Sketch ribbon: the tangent arc tool.
    ToolArcTangent => ("접선 호", "Tangent Arc"),
    /// Sketch ribbon: the Three Point Rectangle tool.
    ToolRectangle3 => ("3점 직사각형", "Three Point Rectangle"),
    /// Browser menu: makes a representation the active one.
    CmdActivate => ("활성화", "Activate"),
    /// Browser, right-click a joint: lock it where it stands.
    JointLockItem => ("잠금", "Lock"),
    /// File menu: the Application Options window.
    MenuOptions => ("응용 프로그램 옵션", "Application Options"),
    /// Application Options, Sketch tab: project the edges of the selected face when a sketch is created.
    OptSnapToGrid => ("격자에 스냅", "Snap to grid"),
    OptAutoBend => ("3D 선 그릴 때 자동 굽힘", "Auto-bend with 3D line creation"),
    OptAutoproject => ("스케치 작성·편집 시 모서리 자동 투영", "Autoproject edges for sketch creation and edit"),
    /// 3D Model tab: group of the work features.
    GroupWorkFeatures => ("작업 피처", "Work Features"),
    /// 3D Model tab: creates a work axis; the dialog's title.
    CmdWorkAxis => ("작업 축", "Work Axis"),
    /// Work axis dialog: along a sketch line.
    WorkAxisTitle => ("작업 축", "Work Axis"),
    /// Work axis type: On Line or Edge.
    WpParallelThrough => ("평면에 평행, 점 통과", "Parallel to Plane Through Point"),
    WpMidplane => ("두 평면의 중간면", "Midplane Between Two Planes"),
    WpAngle => ("평면에 대한 각도 (선 기준)", "Angle to Plane Around Edge"),
    WpLinePoint => ("선과 점", "Line and Point"),
    WpNormalLine => ("선에 수직, 점 통과", "Normal to Axis Through Point"),
    WpNormalCurve => ("곡선에 수직, 점 통과", "Normal to Curve at Point"),
    WpPointLines => ("두 선의 교차", "Intersection of Two Lines"),
    WpPointPlaneLine => ("평면과 선의 교차", "Intersection of Plane and Line"),
    WpThreePlanes => ("세 평면의 교차", "Intersection of Three Planes"),
    WpCurveCentre => ("곡선의 중심", "Center of a Curve"),
    WaFirstLine => ("첫 번째 선", "First line"),
    WaSecondLine => ("두 번째 선", "Second line"),
    WaThirdPlane => ("세 번째 평면", "Third plane"),
    WaCurve => ("곡선", "Curve"),
    WaAngleDeg => ("각도 (°)", "Angle (°)"),
    WaOnLine => ("선 위", "On Line"),
    /// Work axis type: Parallel to Line Through Point.
    WaParallel => ("선에 평행, 점 통과", "Parallel to Line Through Point"),
    /// Work axis type: Through Two Points.
    WaTwoPoints => ("두 점 통과", "Through Two Points"),
    /// Work axis type: Intersection of Two Planes.
    WaTwoPlanes => ("두 평면의 교선", "Intersection of Two Planes"),
    /// Work axis type: Through Center of Circular Shape.
    WaCircle => ("원의 중심 통과", "Through Center of Circular Shape"),
    /// Work axis dialog: pick a sketch line.
    WaLine => ("선", "Line"),
    /// Work axis dialog: pick a point.
    WaPoint => ("점", "Point"),
    /// Work axis dialog: the first point.
    WaFirstPoint => ("첫 번째 점", "First point"),
    /// Work axis dialog: the second point.
    WaSecondPoint => ("두 번째 점", "Second point"),
    /// Work axis dialog: the first plane.
    WaFirstPlane => ("첫 번째 평면", "First plane"),
    /// Work axis dialog: the second plane.
    WaSecondPlane => ("두 번째 평면", "Second plane"),
    /// Work axis dialog: pick a circle or arc.
    WaCircleOf => ("원·호", "Circle or arc"),
    /// Browser: the folder of work features.
    BrowserWorkFeatures => ("작업 피처", "Work Features"),
    /// 3D Model tab: creates a work plane.
    CmdWorkPlane => ("작업 평면", "Work Plane"),
    /// 3D Model tab: creates a work point.
    CmdWorkPoint => ("작업 점", "Work Point"),
    /// Work plane dialog title.
    WorkPlaneTitle => ("작업 평면", "Work Plane"),
    /// Work point dialog title.
    WorkPointTitle => ("작업 점", "Work Point"),
    /// Work plane type: Offset from Plane.
    WpOffset => ("평면에서 간격띄우기", "Offset from Plane"),
    /// Work plane type: Three Points.
    WpThreePoints => ("세 점", "Three Points"),
    /// Work axis type: Normal to Plane Through Point.
    WaNormal => ("평면에 수직, 점 통과", "Normal to Plane Through Point"),
    /// Work feature dialog: the plane an offset starts from, or the plane of a normal axis.
    WaBasePlane => ("평면", "Plane"),
    /// Work feature dialog: the offset distance in millimetres.
    WaDistance => ("거리 (mm)", "Distance (mm)"),
    /// Work feature dialog: the third point.
    WaThirdPoint => ("세 번째 점", "Third point"),
    /// Work point dialog: give the position by coordinates.
    WaTypedPoint => ("좌표로 입력", "Type the coordinates"),
    /// Browser, right-click a work feature: change its definition.
    CmdEditWorkFeature => ("정의 편집", "Edit definition"),
    /// Sketch ribbon, Modify panel: cuts a curve back to its nearest crossings.
    ToolTrim => ("자르기", "Trim"),
    /// Sketch ribbon, Modify panel: lengthens a line or arc to the next curve.
    ToolExtend => ("연장", "Extend"),
    /// Sketch ribbon: rounds the corner between two lines.
    ToolFillet => ("모깎기", "Fillet"),
    /// Dimension edit box: a driven (reference) dimension measures without constraining.
    DimDriven => ("종속 치수", "Driven dimension"),
    /// Browser, right-click a component or subassembly: edit it in place.
    CmdEditInPlace => ("제자리 편집", "Edit"),
    /// Browser, right-click a component: group it into a new subassembly.
    CmdDemote => ("서브어셈블리로 묶기 (강등)", "Component Demote"),
    /// Browser: move a component or subassembly to the top level.
    CmdMoveToTop => ("최상위로 이동", "Move to top level"),
    /// Browser: move a component into another subassembly.
    CmdMoveInto => ("서브어셈블리로 이동", "Move into subassembly"),
    /// Browser, right-click a subassembly: dissolve it; its contents go up one level.
    CmdDissolve => ("해체", "Dissolve"),
    CmdInsertSplinePoint => ("점 삽입", "Insert Point"),
    CmdCloseSpline => ("곡선 닫기", "Close Curve"),
    SplineInsertHint => ("스플라인 위를 클릭해 점을 삽입합니다 (Esc로 끝)", "Click the spline to insert points (Esc to end)"),
    ToolHelix => ("나선 곡선", "Helical Curve"),
    HelixPitchRevolution => ("피치 및 회전", "Pitch and Revolution"),
    HelixRevolutionHeight => ("회전 및 높이", "Revolution and Height"),
    HelixPitchHeight => ("피치 및 높이", "Pitch and Height"),
    HelixSpiral => ("나선형", "Spiral"),
    HelixPitch => ("피치", "Pitch"),
    HelixRevolutions => ("회전", "Revolutions"),
    HelixHeight => ("높이", "Height"),
    HelixDiameter => ("지름", "Diameter"),
    HelixTaper => ("테이퍼", "Taper"),
    HelixRightHand => ("오른손", "Right hand"),
    HelixLeftHand => ("왼손", "Left hand"),
    HelixPickStart => ("나선 곡선: 시작점을 클릭하십시오", "Helical Curve: click the start point"),
    HelixPickEnd => ("나선 곡선: 끝점(축 방향)을 클릭하십시오", "Helical Curve: click the end point"),
    ToolBend => ("굽힘", "Bend"),
    BendPickFirst => ("굽힘: 첫 번째 선을 클릭하십시오", "Bend: click the first line"),
    BendPickSecond => ("굽힘: 두 번째 선을 클릭하십시오", "Bend: click the second line"),
    BendRadius => ("반지름", "Radius"),
    CmdSplitSpline => ("스플라인 분할", "Split Spline"),
    CmdFlexible => ("유연", "Flexible"),
    /// Assemble tab, Edit group: the group's title.
    GroupEdit => ("편집", "Edit"),
    /// Assemble tab: back to the previous edit target.
    CmdReturn => ("복귀", "Return"),
    /// Assemble tab: edit the parent of the edit target.
    CmdReturnParent => ("상위로", "Return to Parent"),
    /// Assemble tab: back to the top-level assembly.
    QatTitle => ("빠른 실행", "Quick Access"),
    CmdReturnTop => ("최상위로", "Return to Top"),
    /// Browser, right-click a joint: refuse relationships that take its freedom away.
    JointProtectItem => ("보호", "Protect"),
    JointLimitsItem => ("한계…", "Limits…"),
    LimitAngular => ("각도 한계", "Angular limits"),
    LimitLinear => ("선형 한계", "Linear limits"),
    LimitStart => ("시작", "Start"),
    LimitCurrent => ("현재", "Current"),
    LimitEnd => ("끝", "End"),
    LimitNone => ("한계를 쓸 수 있는 조인트가 아닙니다 (고정 조인트이거나 아직 원점을 고르지 않았습니다)", "No limits here (a rigid joint, or the origins are not picked yet)"),
    JointTabPlacement => ("조인트", "Joint"),
    JointTabLimits => ("한계", "Limits"),
    LimitNow => ("현재 위치: {0}", "Now at {0}"),
    /// Assemble tab: the Assemble command (pick geometry, the geometry decides the constraint types).
    AssembleTitle => ("어셈블", "Assemble"),
    /// Assemble command: the two selections allow no constraint.
    AssembleNone => ("이 형상 조합에는 구속이 없습니다.", "No constraint fits this geometry."),
    /// Assemble command: the angle field.
    AssembleAngle => ("각도", "Angle"),
    /// Assemble command: deletes the selections and goes on.
    AssembleUndo => ("선택 지우기", "Clear selections"),
    /// Assemble command solution: mate with faces facing.
    SolMateMate => ("맞춤 (마주봄)", "Mate (opposed)"),
    /// Assemble command solution: flush.
    SolMateFlush => ("플러시", "Flush"),
    /// Assemble command solution: mate of axes pointing the same way.
    SolMateAligned => ("맞춤 (정렬)", "Mate (aligned)"),
    /// Assemble command solution: directed angle.
    SolAngleDirected => ("각도 (방향)", "Angle (directed)"),
    /// Assemble command solution: tangent inside.
    SolTangentInside => ("접선 (안쪽)", "Tangent (inside)"),
    /// Assemble command solution: tangent outside.
    SolTangentOutside => ("접선 (바깥쪽)", "Tangent (outside)"),
    /// Assemble command solution: insert, opposed.
    SolInsertOpposed => ("삽입 (반대)", "Insert (opposed)"),
    /// Assemble command solution: insert, aligned.
    SolInsertAligned => ("삽입 (정렬)", "Insert (aligned)"),
    /// Plan card: what the checks guarantee. `{0}` padding mm, `{1}` step mm, `{2}` step degrees, `{3}` clearance mm.
    PlanGuarantee => ("패딩 {0} mm, 검사 간격 {1} mm·{2}°에서 볼록 충돌 모델이 장애물과 {3} mm 이상 떨어짐", "Padding {0} mm; with check steps of {1} mm and {2} degrees the convex collision model stays at least {3} mm from obstacles"),
    /// Plan card: the guarantee holds only at the checked states.
    PlanGuaranteeSampled => ("검사한 상태에서만 확인됨(상태 사이는 보증하지 않음)", "Checked at the sampled states only (nothing is guaranteed between them)"),
    /// Plan card: what the guarantee leaves out.
    PlanGuaranteeExcluded => ("제외: 검사하지 않는 쌍(ACM), 닫힌 메시 안에서 시작한 바디, 컨트롤러 추종 오차", "Not covered: pairs that are not checked (ACM), bodies that start inside a closed mesh, controller tracking error"),
    /// Plan card: contact allowances lower the padding.
    PlanGuaranteeContact => ("접촉 허용 구간에서는 패딩 0", "Padding is 0 where contact is allowed"),
    /// Robot properties: the list of link pairs that are checked or not.
    PanelAcm => ("충돌 검사 쌍(ACM)", "Collision pairs (ACM)"),
    /// Reason a link pair is not checked: joined by a joint.
    AcmAdjacent => ("인접", "adjacent"),
    /// Reason a link pair is not checked: touching in the zero pose.
    AcmTouching => ("영자세에서 닿음", "touching at zero pose"),
    /// A link pair that is checked.
    AcmChecked => ("검사", "checked"),
    /// A link pair as shown in the list. `{0}` and `{1}` are link names.
    AcmPair => ("{0} - {1}", "{0} - {1}"),
    /// Path properties: contact allowances.
    PanelContacts => ("접촉 허용", "Contact allowances"),
    /// A contact allowance. `{0}` step number, `{1}` link, `{2}` body.
    ContactRow => ("{0}번째 이동: {1} - {2}", "Move {0}: {1} - {2}"),
    /// Contact allowance option: along the whole motion.
    ContactWhole => ("이동 전체", "Whole motion"),
    /// Contact allowance option: overlap allowed.
    ContactTouch => ("겹침 허용", "Allow overlap"),
    /// Contact allowance form: the move.
    ContactMove => ("이동", "Move"),
    /// Contact allowance form: the robot link.
    ContactLink => ("링크", "Link"),
    /// Contact allowance form: the obstacle.
    ContactObstacle => ("장애물", "Obstacle"),
    /// Contact allowance form: add.
    ContactAdd => ("접촉 허용 추가", "Allow contact"),
    /// Sketch tools that are planned and shown disabled until the document supports them.
    /// Ribbon group of sketch tools that are not available yet.
    ToolPoint => ("점", "Point"),
    ToolPolygon => ("다각형", "Polygon"),
    ToolSlot => ("슬롯", "Slot"),
    /// Context menu of a sketch in the browser.
    CmdEditSketch => ("스케치 편집", "Edit sketch"),
    /// Tools menu: the performance overlay of the viewport.
    MenuPerfOverlay => ("성능 오버레이", "Performance overlay"),
    /// The overlay's text. `{0}` frame time in ms, `{1}` frames per second, `{2}` draw calls, `{3}` triangles.
    PerfStats => ("프레임 {0} ms ({1} fps) · 드로 {2} · 삼각형 {3}", "Frame {0} ms ({1} fps) · draws {2} · triangles {3}"),
    // --- Measure (Inventor: Tools tab > Measure panel > Measure) ---
    /// Tools menu and the panel title.
    MenuMeasure => ("측정", "Measure"),
    /// Measure: what to click. Shown until two items are selected.
    MeasureHint => ("뷰포트나 브라우저에서 측정할 항목을 선택하십시오. 빈 곳을 클릭하면 다시 시작합니다.", "Select items to measure in the view or the browser. Click a blank area to restart."),
    /// Measure: heading of selection 1 / 2.
    MeasureFirst => ("선택 1", "Selection 1"),
    MeasureSecond => ("선택 2", "Selection 2"),
    /// Measure: position of an item's origin. `{0}`, `{1}`, `{2}` x, y, z in mm.
    MeasurePosition => ("위치 X {0}  Y {1}  Z {2} mm", "Position X {0}  Y {1}  Z {2} mm"),
    MeasureMinimum => ("최소 거리", "Minimum Distance"),
    MeasureCentres => ("중심 간 거리", "Center to Center"),
    MeasureMaximum => ("최대 거리", "Maximum Distance"),
    /// Measure: the delta between the origins. `{0}`, `{1}`, `{2}` dx, dy, dz in mm.
    MeasureDelta => ("Δ X {0}  Y {1}  Z {2} mm", "Δ X {0}  Y {1}  Z {2} mm"),
    /// Measure: angle between the Z axes. `{0}` degrees.
    MeasureAngle => ("Z축 사이 각도 {0}°", "Angle between Z axes {0}°"),
    /// Measure: the minimum distance could not be measured (a path, or a missing mesh).
    MeasureNoMinimum => ("최소 거리 측정 불가", "Minimum distance not available"),
    MeasureCopyAll => ("모두 복사", "Copy All"),
    /// Measure: add the value to / subtract it from the accumulated values.
    MeasureAdd => ("누적 값에 추가", "Add to Accumulated Value"),
    MeasureSubtract => ("누적 값에서 빼기", "Subtract from Accumulated Value"),
    /// Measure: heading of the accumulated values.
    MeasureAccumulated => ("누적 특성", "Accumulated Properties"),
    MeasureArea => ("면적", "Area"),
    MeasureAngleGroup => ("각도", "Angle"),
    MeasureDeleteValue => ("이 값 삭제", "Delete this value"),
    MeasureDeleteGroup => ("이 그룹 삭제", "Delete this group"),
    /// Measure: selection priority (Inventor: Component / Edges and Faces).
    MeasureByComponent => ("컴포넌트 우선", "Component Priority"),
    MeasureByFeature => ("모서리 및 면 우선", "Edges and Faces"),
    /// Measure: a picked corner or point on a surface. `{0}`, `{1}`, `{2}` x, y, z in mm.
    MeasurePoint => ("점 X {0}  Y {1}  Z {2} mm", "Point X {0}  Y {1}  Z {2} mm"),
    /// Measure: a picked straight edge. `{0}` its length in mm.
    MeasureEdge => ("모서리 길이 {0} mm", "Edge length {0} mm"),
    /// Measure: a picked flat face. `{0}` its area in mm².
    MeasureFace => ("면 면적 {0} mm²", "Face area {0} mm²"),
    MeasureDistance => ("거리", "Distance"),
    /// Measure: a picked cylindrical face. `{0}` diameter, `{1}` radius, `{2}` length, all mm.
    MeasureCylinder => ("원통 지름 {0} mm · 반지름 {1} mm · 길이 {2} mm", "Cylinder diameter {0} mm · radius {1} mm · length {2} mm"),
    /// Measure: the angle between two picked features. `{0}` degrees.
    MeasureFeatureAngle => ("각도 {0}°", "Angle {0}°"),
    MeasureRestart => ("다시 시작", "Restart"),
    MeasureDone => ("완료", "Done"),
    // --- Analyze Interference (Inventor: Inspect tab > Interference panel) ---
    /// Tools menu and the window title.
    MenuInterference => ("간섭 분석", "Analyze Interference"),
    /// Interference: the buttons that define a set; while one is active, picks add to (or remove from) it.
    InterferenceSet1 => ("세트 #1 정의", "Define Set #1"),
    InterferenceSet2 => ("세트 #2 정의", "Define Set #2"),
    /// Interference: put every robot and body in set 1 (Inventor's tip: a crossing window over all).
    InterferenceAll => ("모든 컴포넌트를 세트 #1에", "All components in Set #1"),
    /// Interference: what to do while defining a set.
    InterferenceHint => ("세트 버튼을 누른 뒤 뷰포트나 브라우저에서 컴포넌트를 선택하십시오. 다시 선택하면 빠집니다. 세트 #2가 비어 있으면 세트 #1 안에서 서로 검사합니다.", "Press a set button, then select components in the view or the browser; selecting one again removes it. With Set #2 empty, Set #1 is checked among itself."),
    /// Interference: the summary. `{0}` the count, `{1}` the total volume in mm³.
    InterferenceFound => ("간섭 {0}개 · 총 체적 {1} mm³", "{0} interferences · total volume {1} mm³"),
    InterferenceNone => ("간섭이 없습니다.", "No interference detected."),
    /// Interference: one row. `{0}` and `{1}` the components, `{2}` the volume in mm³, `{3}` `{4}` `{5}` the centroid in mm.
    InterferenceRow => ("{0} : {1}  체적 {2} mm³  중심 ({3}, {4}, {5}) mm", "{0} : {1}  volume {2} mm³  centroid ({3}, {4}, {5}) mm"),
    InterferenceShowIgnored => ("무시한 항목 표시", "Ignored"),
    InterferenceIgnore => ("무시", "Ignore"),
    /// Interference: ignore every interference with a volume below this one.
    InterferenceIgnoreSmaller => ("이 체적 이하 모두 무시", "Ignore all volumes up to this one"),
    InterferenceUnignore => ("무시 해제", "Un-ignore"),
    InterferenceUnignoreAll => ("모두 무시 해제", "Un-ignore all"),
    /// Interference: back to the sets.
    InterferenceAgain => ("다시 분석", "Analyze again"),
    /// Interference: the volumes are estimated on a grid.
    InterferenceEstimate => ("체적은 격자 표본으로 추정합니다(충돌 형상 기준). 항목을 두 번 클릭하면 확대합니다.", "Volumes are estimated on a sample grid (collision shapes). Double-click an entry to zoom to it."),
    // --- Drive (Inventor: right-click a relationship > Drive) ---
    /// Tools menu and the window title.
    MenuDrive => ("구동", "Drive"),
    DriveRobot => ("로봇", "Robot"),
    DriveJoint => ("관절", "Joint"),
    DriveStart => ("시작", "Start"),
    DriveEnd => ("끝", "End"),
    /// Drive: seconds between steps.
    DrivePause => ("일시 중지 지연(초)", "Pause Delay (s)"),
    DriveMore => ("자세히", "More"),
    DriveCollision => ("충돌 감지", "Collision Detection"),
    DriveIncrement => ("증분", "Increment"),
    DriveByValue => ("값의 양", "Amount of Value"),
    DriveBySteps => ("총 단계 수", "Total # of Steps"),
    DriveRepetitions => ("반복", "Repetitions"),
    DriveStartEnd => ("시작/끝", "Start/End"),
    DriveStartEndStart => ("시작/끝/시작", "Start/End/Start"),
    DriveForward => ("정방향", "Forward"),
    DriveReverse => ("역방향", "Reverse"),
    DriveStop => ("중지", "Stop"),
    DriveStepForward => ("한 단계 앞으로", "Single Step Forward"),
    DriveStepReverse => ("한 단계 뒤로", "Single Step Reverse"),
    DriveToStart => ("시작으로", "Go to Start"),
    DriveToEnd => ("끝으로", "Go to End"),
    /// Drive: the joint value now. `{0}` the value with its unit.
    DriveValue => ("현재 값 {0}", "Value {0}"),
    /// Drive: collision detection stopped the motion. `{0}` what collided, `{1}` the value.
    DriveCollided => ("충돌 감지: {0} ({1}에서 멈춤)", "Collision detected: {0} (stopped at {1})"),
    /// Drive: what is driven — a robot joint, or an assembly relationship.
    DriveRelation => ("구동 대상", "Drive"),
    DriveTargetJoint => ("로봇 관절", "Robot joint"),
    /// Drive: a relationship could not be driven. `{0}` the reason.
    DriveRefused => ("구동할 수 없음: {0}", "Cannot drive: {0}"),
    /// Drive: collision detection found two components overlapping. `{0}`, `{1}` the components, `{2}` the value.
    DriveInterfered => ("충돌 감지: {0} ↔ {1} ({2}에서 멈춤)", "Collision detected: {0} ↔ {1} (stopped at {2})"),
    // --- Parameters (Inventor: Manage tab > Parameters panel > Parameters) ---
    /// Tools menu and the dialog title.
    BomTitle => ("자재 명세서(BOM)", "Bill of Materials"),
    BomStructured => ("구조화", "Structured"),
    BomPartsOnly => ("부품 전용", "Parts Only"),
    BomItem => ("항목", "Item"),
    BomPartNumber => ("부품 번호", "Part Number"),
    BomQuantity => ("수량", "Quantity"),
    BomDescription => ("설명", "Description"),
    BomStructureColumn => ("BOM 구조", "BOM Structure"),
    BomExport => ("CSV로 내보내기…", "Export to CSV…"),
    BomExported => ("{0}에 저장했습니다", "Saved to {0}"),
    BomEmpty => ("어셈블리에 컴포넌트가 없습니다.", "The assembly has no components."),
    CmdFindInWindow => ("창에서 찾기", "Find in Window"),
    CmdIProperties => ("iProperties…", "iProperties…"),
    IpTitleDocument => ("iProperties — 문서", "iProperties — Document"),
    IpTabSummary => ("요약", "Summary"),
    IpTabProject => ("프로젝트", "Project"),
    IpTabPhysical => ("물리적", "Physical"),
    IpTitle => ("제목", "Title"),
    IpSubject => ("주제", "Subject"),
    IpAuthor => ("작성자", "Author"),
    IpCompany => ("회사", "Company"),
    IpComments => ("설명", "Comments"),
    IpPartNumber => ("부품 번호", "Part Number"),
    IpDescription => ("설명(부품)", "Description"),
    IpRevision => ("개정 번호", "Revision Number"),
    IpProject => ("프로젝트", "Project"),
    IpDesigner => ("설계자", "Designer"),
    IpVendor => ("공급업체", "Vendor"),
    IpDensity => ("밀도(kg/m³)", "Density (kg/m^3)"),
    IpMassOverride => ("질량 재정의(kg)", "Mass override (kg)"),
    IpMass => ("질량", "Mass"),
    IpVolume => ("부피", "Volume"),
    IpArea => ("면적", "Area"),
    IpCentre => ("무게중심", "Center of Gravity"),
    IpUpdate => ("업데이트", "Update"),
    IpNotKnown => ("이 항목의 모양을 아직 알 수 없어 계산할 수 없습니다.", "The shape of this item is not known yet, so nothing can be calculated."),
    IpOverridden => ("(재정의됨)", "(overridden)"),
    MenuDocSettings => ("문서 설정", "Document Settings"),
    DsTabUnits => ("단위", "Units"),
    DsTabSketch => ("스케치", "Sketch"),
    DsLength => ("길이", "Length"),
    DsAngle => ("각도", "Angle"),
    DsLinearPrecision => ("선형 자릿수", "Linear precision"),
    DsAngularPrecision => ("각도 자릿수", "Angular precision"),
    DsDimensionDisplay => ("치수 표시", "Dimension display"),
    DsDisplayValue => ("값", "Value"),
    DsDisplayName => ("이름", "Name"),
    DsDisplayExpression => ("식", "Expression"),
    DsDisplayPrecise => ("정밀 값", "Precise Value"),
    DsInputExpression => ("입력 상자에 식을 표시", "Show the expression in input boxes"),
    DsSnapX => ("스냅 간격 X", "Snap spacing X"),
    DsSnapY => ("스냅 간격 Y", "Snap spacing Y"),
    DsSnapsPerMinor => ("보조선당 스냅 수", "Snaps per minor"),
    DsMinorPerMajor => ("주선당 보조선 수", "Minor lines per major"),
    DsAutoBendRadius => ("3D 스케치 자동 굽힘 반지름", "3D Sketch Auto-Bend Radius"),
    DsInfer => ("구속조건 추론", "Infer constraints"),
    DsPersist => ("추론한 구속조건 유지", "Persist constraints"),
    DsInvalid => ("값이 올바르지 않습니다(0보다 큰 길이)", "A value is not valid (a length above zero)"),
    DsApply => ("적용", "Apply"),
    MenuParameters => ("매개변수", "Parameters"),
    ParamSearch => ("검색", "Search"),
    ParamModel => ("모델 매개변수", "Model Parameters"),
    ParamUser => ("사용자 매개변수", "User Parameters"),
    ParamColName => ("매개변수 이름", "Parameter Name"),
    ParamColUnit => ("단위", "Unit"),
    ParamColEquation => ("수식", "Equation"),
    ParamColValue => ("공칭 값", "Nominal Value"),
    /// Parameters: add a numeric user parameter.
    ParamAddNumeric => ("숫자 추가", "Add Numeric"),
    /// Parameters: the unit of a new parameter.
    ParamUnitLength => ("mm", "mm"),
    ParamUnitAngle => ("deg", "deg"),
    ParamDelete => ("매개변수 삭제", "Delete Parameter"),
    ParamCopyToUser => ("사용자 매개변수에 복사", "Copy to User Parameter"),
    ParamDone => ("완료", "Done"),
    /// Parameters: a model parameter's name: the sketch and what the dimension measures. `{0}` sketch, `{1}` kind, `{2}` number.
    ParamDimension => ("{0} · {1} {2}", "{0} · {1} {2}"),
    ParamKindLength => ("거리", "distance"),
    ParamKindAngle => ("각도", "angle"),
    ParamKindRadius => ("반지름", "radius"),
    ParamKindDiameter => ("지름", "diameter"),
    /// Parameters: a reference (driven) dimension's equation cannot be edited.
    ParamDriven => ("(참조)", "(reference)"),
    /// Assemble tab > Position: ground or free the selected component (Inventor's Grounded).
    CmdGround => ("접지", "Grounded"),
    GripTitle => ("그립 스냅", "Grip Snap"),
    SelPriorityComponent => ("구성요소 우선", "Component priority"),
    SelPriorityFeature => ("작업 피처 우선", "Work feature priority"),
    SelOther => ("다른 항목 선택", "Select Other"),
    GripMove => ("축을 따라 이동", "Move along ray"),
    GripRotate => ("선 기준 회전", "Rotate about line"),
    GripPickEdge => ("기준이 될 구성요소의 모서리·꼭짓점·평면을 선택하세요", "Select an edge, corner or flat face of the component as the reference"),
    GripMovePoint => ("점 기준 이동", "Move using the point"),
    GripRotatePoint => ("점 기준 회전", "Rotate about the point"),
    GripMovePlane => ("면 안에서 이동", "Move within the plane"),
    GripMoveNormal => ("법선 방향 이동", "Move along the normal"),
    GripPickFrom => ("시작점을 클릭하세요", "Click the start point"),
    GripPickTo => ("끝점을 클릭하면 위치가 확정됩니다", "Click the end point to fix the result"),
    GripReady => ("완료 또는 접지 후 완료를 누르세요", "Press Done or Commit and Ground"),
    GripAllowed => ("남은 자유도가 이 동작을 허용합니다", "The remaining freedom allows this snap"),
    GripBlocked => ("남은 자유도가 이 동작을 허용하지 않습니다", "The remaining freedom does not allow this snap"),
    GripDone => ("완료", "Done"),
    GripGround => ("접지 후 완료", "Commit and Ground"),
    GripCancel => ("취소", "Cancel"),
    /// Status: the selected component was grounded / freed. `{0}` its name.
    GroundOn => ("{0}을(를) 접지했습니다.", "{0} is grounded."),
    GroundOff => ("{0}의 접지를 풀었습니다.", "{0} is no longer grounded."),
    /// Status: an assembly file is being read; `{0}` meshes are still loading.
    ImportLoading => ("불러오는 중… (메시 {0}개 남음)", "Loading… ({0} meshes to go)"),
    /// Status: reading an assembly file failed. `{0}` why.
    ImportFailed => ("가져오기 실패: {0}", "Import failed: {0}"),
    // --- Project Cut Edges (Inventor: Sketch tab > Create panel) ---
    ToolCutEdges => ("절단 모서리 투영", "Project Cut Edges"),
    /// Status: how many cut edges were projected. `{0}` the count.
    CutEdgesDone => ("절단 모서리 {0}개를 투영했습니다.", "Projected {0} cut edges."),
    /// Status: the sketch plane cuts nothing.
    CutEdgesNone => ("스케치 평면이 자르는 모서리가 없습니다.", "The sketch plane cuts no edges."),
    // --- AI agent API ---
    /// Tools menu: the window that connects AI agents (MCP).
    MenuAgent => ("AI 에이전트 연결", "AI agent connection"),
    /// Agent window: the switch that starts the local MCP server.
    AgentEnable => ("에이전트 연결 허용 (이 PC에서만)", "Allow agent connections (this PC only)"),
    /// Agent window: what the server does.
    AgentExplain => ("Claude Code·Claude Desktop 같은 AI 에이전트가 MCP로 이 창의 셀을 읽고 편집합니다. 모든 편집은 Ctrl+Z로 되돌릴 수 있습니다.", "AI agents such as Claude Code or Claude Desktop read and edit this window's cell over MCP. Every edit can be undone with Ctrl+Z."),
    /// Agent window: the server's address. `{0}` the URL.
    AgentAddress => ("주소: {0}", "Address: {0}"),
    AgentPort => ("포트", "Port"),
    AgentToken => ("접근 토큰", "Access token"),
    AgentNewToken => ("새 토큰", "New token"),
    AgentCopy => ("복사", "Copy"),
    /// Agent window: heading of the command that registers GraphCTRL with Claude Code.
    AgentClaudeCode => ("Claude Code에 등록하는 명령", "Command to register with Claude Code"),
    /// Agent window: heading of the activity log.
    AgentActivity => ("에이전트 작업 기록", "Agent activity"),
    /// Agent window: the server could not start. `{0}` the reason.
    AgentFailed => ("에이전트 서버를 시작하지 못했습니다: {0}", "The agent server could not start: {0}"),
    /// Status bar: an agent edited the cell. `{0}` what it did.
    AgentEdited => ("AI 에이전트: {0}", "AI agent: {0}"),
    // --- backend: sketch Point, Polygon, Slot, Mirror, Patterns, Offset (Inventor: Sketch tab) ---
    PromptPointPlace => ("점을 놓을 곳을 클릭하십시오", "Click to place a point"),
    PromptPolygonCentre => ("다각형의 중심을 클릭하십시오", "Click the centre of the polygon"),
    PromptPolygonCorner => ("꼭짓점(내접) 또는 변의 중점(외접)을 클릭하십시오", "Click a vertex (inscribed) or an edge midpoint (circumscribed)"),
    PromptSlotFirst => ("슬롯의 첫 점을 클릭하십시오", "Click the first point of the slot"),
    PromptSlotSecond => ("슬롯의 두 번째 점을 클릭하십시오", "Click the second point of the slot"),
    PromptSlotWidth => ("슬롯 너비를 클릭하거나 입력하십시오", "Click or type the slot width"),
    PromptOffsetPick => ("간격띄우기할 곡선을 선택하십시오", "Select the curve to offset"),
    PromptOffsetPlace => ("간격띄우기 위치를 클릭하거나 거리를 입력하십시오", "Click where the offset goes, or type the distance"),
    PromptPickGeometry => ("패턴할 형상을 클릭하십시오(다시 클릭하면 빠짐)", "Click the geometry to pattern (click again to remove)"),
    PromptPickLine => ("선을 클릭하십시오", "Click a line"),
    PromptArcSlotCentre => ("슬롯 중심 호의 중심을 클릭하십시오", "Click the centre of the slot's centre arc"),
    PromptArcSlotThrough => ("슬롯 중심 호가 지나는 점을 클릭하십시오", "Click a point on the slot's centre arc"),
    PromptArcSlotEnd => ("슬롯 중심 호의 끝을 클릭하십시오", "Click the end of the slot's centre arc"),
    ToolSlotThreePointArc => ("3점 호 슬롯", "Three Point Arc Slot"),
    ToolSlotCenterPointArc => ("중심점 호 슬롯", "Center Point Arc Slot"),
    PromptPickPoint => ("점이나 원을 클릭하십시오(원은 중심)", "Click a point or a circle (its centre)"),
    /// Input box: offset distance.
    HudDistance => ("거리", "Distance"),
    HudX => ("X", "X"),
    ToolArc3 => ("3점 호", "Three Point Arc"),
    CmdCancelSketch => ("스케치 취소", "Cancel Sketch"),
    PromptMidpointLineMid => ("선의 중점을 클릭하십시오", "Click the midpoint of the line"),
    PromptMidpointLineEnd => ("선의 끝을 클릭하십시오", "Click the end of the line"),
    PromptCenterRectCentre => ("직사각형의 중심을 클릭하십시오", "Click the centre of the rectangle"),
    PromptCenterRectCorner => ("직사각형의 모서리를 클릭하십시오", "Click a corner of the rectangle"),
    PromptCenterRectSide => ("한 변의 가운데를 클릭하십시오", "Click the middle of one side"),
    PromptCenterRectWidth => ("나머지 변의 거리를 클릭하십시오", "Click the distance of the other sides"),
    PromptEllipseCentre => ("타원의 중심을 클릭하십시오", "Click the centre of the ellipse"),
    PromptEllipseAxis => ("첫째 축의 끝을 클릭하십시오", "Click the end of the first axis"),
    PromptEllipseMinor => ("타원 위의 한 점을 클릭하십시오", "Click a point on the ellipse"),
    ToolEllipse => ("타원", "Ellipse"),
    PromptSplineFirst => ("스플라인의 첫 맞춤점을 클릭하십시오", "Click the first fit point of the spline"),
    PromptSplineNext => (
        "다음 맞춤점을 클릭하십시오(Enter 또는 마지막 점 다시 클릭으로 마침)",
        "Click the next fit point (Enter, or the last point again, to finish)"
    ),
    ToolSpline => ("스플라인(보간)", "Spline (Interpolation)"),
    PromptTangentCircleLine => ("원이 접할 선을 클릭하십시오(3개)", "Click a line the circle touches (three)"),
    ToolMidpointLine => ("중점 선", "Midpoint Line"),
    ToolCenterRectangle => ("2점 중심 직사각형", "Two Point Center Rectangle"),
    ToolCenterRectangle3 => ("3점 중심 직사각형", "Three Point Center Rectangle"),
    ToolTangentCircle => ("접선 원", "Tangent Circle"),
    PromptSplitPick => ("분할할 곡선을 클릭하십시오(가장 가까운 교차점에서 나뉨)", "Click the curve to split (at the nearest crossing)"),
    PromptChamferFirst => ("모따기할 첫 선을 클릭하십시오", "Click the first line to chamfer"),
    PromptChamferSecond => ("두 번째 선을 클릭하십시오", "Click the second line"),
    ToolSplit => ("분할", "Split"),
    ToolChamfer => ("모따기", "Chamfer"),
    ChamferTitle => ("2D 모따기", "2D Chamfer"),
    ChamferEqual => ("같은 거리", "Equal Distance"),
    ChamferUnequal => ("다른 거리", "Unequal Distance"),
    ChamferDistanceAngle => ("거리와 각도", "Distance and Angle"),
    ChamferDistance => ("거리", "Distance"),
    ChamferDistance1 => ("거리 1", "Distance 1"),
    ChamferDistance2 => ("거리 2", "Distance 2"),
    ChamferAngle => ("각도", "Angle"),
    ChamferCreateDimensions => ("치수 만들기", "Create Dimensions"),
    PromptMoveBase => ("기준점을 클릭하십시오(선택한 형상)", "Click the base point (of the selection)"),
    PromptMoveEnd => ("끝점을 클릭하십시오", "Click the end point"),
    PromptRotateCentre => ("회전 중심점을 클릭하십시오(선택한 형상)", "Click the centre point (of the selection)"),
    PromptRotateEnd => ("끝점을 클릭하거나 각도를 입력하십시오", "Click the end point, or type the angle"),
    ToolMove => ("이동", "Move"),
    ToolCopy => ("복사", "Copy"),
    ToolRotate => ("회전", "Rotate"),
    /// Move / Copy / Rotate dialog.
    TransformSelection => ("선택: 곡선 {0}개(명령 전에 고르기)", "Selection: {0} curves (pick before the command)"),
    TransformCopy => ("복사본으로", "Copy"),
    TransformRelax => ("구속 풀기(치수 다시 계산·구속 끊기)", "Relax (recompute dimensions, break constraints)"),
    ToolAutoDimension => ("자동 치수 및 구속", "Automatic Dimensions and Constraints"),
    ToolFlipDimension => ("방향 뒤집기", "Flip Direction"),
    PromptScaleBase => ("기준점을 클릭하십시오(선택한 형상)", "Click the base point (of the selection)"),
    PromptScaleEnd => ("끝점을 클릭하거나 축척 비율을 입력하십시오", "Click the end point, or type the scale factor"),
    ToolScale => ("축척", "Scale"),
    ToolStretch => ("신축", "Stretch"),
    HudScale => ("축척 비율", "Scale factor"),
    ErrProtected => ("보호된 조인트의 움직임을 막아 거부했습니다", "Refused: it would stop a protected joint's motion"),
    AutoDimTitle => ("자동 치수", "Auto Dimension"),
    AutoDimDimensions => ("치수", "Dimensions"),
    AutoDimConstraints => ("구속", "Constraints"),
    /// `{0}` the freedom left.
    AutoDimRequired => ("필요한 치수: {0}", "Dimensions Required: {0}"),
    AutoDimCurves => ("곡선: 선택 {0}개(없으면 스케치 전체)", "Curves: {0} selected (none: the whole sketch)"),
    AutoDimRemove => ("제거", "Remove"),
    HudY => ("Y", "Y"),
    /// Ribbon group (Inventor: Pattern panel).
    GroupPattern => ("패턴", "Pattern"),
    /// Ribbon group (Inventor: Modify panel).
    GroupModify => ("수정", "Modify"),
    /// Format toggle: new points are centre points.
    ToolCenterPoint => ("중심점", "Center Point"),
    ToolSlotCenterToCenter => ("중심 대 중심 슬롯", "Center to Center Slot"),
    ToolSlotOverall => ("전체 슬롯", "Overall Slot"),
    ToolSlotCenterPoint => ("중심점 슬롯", "Center Point Slot"),
    ToolMirror => ("미러", "Mirror"),
    ToolRectPattern => ("직사각형", "Rectangular"),
    ToolCircPattern => ("원형", "Circular"),
    ToolOffset => ("간격띄우기", "Offset"),
    /// Polygon dialog.
    PolygonInscribed => ("내접", "Inscribed"),
    PolygonCircumscribed => ("외접", "Circumscribed"),
    PolygonSides => ("변 수", "Edges"),
    /// Offset options (right-click toggles in Inventor).
    OffsetLoopSelect => ("루프 선택", "Loop Select"),
    OffsetConstrain => ("간격띄우기 구속", "Constrain Offset"),
    /// Mirror / pattern dialogs: the Geometry selector. `{0}` how many curves.
    DialogGeometry => ("형상: 곡선 {0}개", "Geometry: {0} curves"),
    DialogSelectorHint => (
        "선택기(형상·선·축)를 누른 뒤 그래픽 창에서 클릭하십시오",
        "Press a selector (geometry, line, axis), then click in the graphics window"
    ),
    /// Mirror dialog: the mirror line. `{0}` its state.
    MirrorLine => ("대칭선: {0}", "Mirror line: {0}"),
    /// Pattern dialogs: a direction or axis is set / not set.
    DialogPicked => ("선택됨", "selected"),
    DialogNotPicked => ("선택 안 됨", "not selected"),
    /// Pattern dialogs: direction 1 / 2. `{0}` its state.
    PatternDirection1 => ("방향 1: {0}", "Direction 1: {0}"),
    PatternDirection2 => ("방향 2: {0}", "Direction 2: {0}"),
    /// Circular pattern: the axis point. `{0}` its state.
    SkPatternAxis => ("축: {0}", "Axis: {0}"),
    /// Sketch patterns: spacing and angle, as expressions with units.
    SkPatternSpacing => ("간격", "Spacing"),
    SkPatternAngle => ("각도", "Angle"),
    /// Sketch pattern dialogs: the second direction's count and spacing.
    SkPatternCount2 => ("개수 (방향 2)", "Count (direction 2)"),
    SkPatternSpacing2 => ("간격 (방향 2)", "Spacing (direction 2)"),
    SkPatternFlip2 => ("뒤집기 (방향 2)", "Flip (direction 2)"),
    /// Window titles (Inventor's dialog names).
    SkMirrorTitle => ("미러 패턴", "Mirror Pattern"),
    SkRectTitle => ("직사각형 패턴", "Rectangular Pattern"),
    SkCircTitle => ("원형 패턴", "Circular Pattern"),
    SkPolygonTitle => ("다각형 옵션", "Polygon options"),
    SkOffsetTitle => ("간격띄우기 옵션", "Offset options"),
    /// Dialog buttons.
    DialogApply => ("적용", "Apply"),
    /// Pattern dialog opened on an existing pattern: OK rebuilds it, Delete Pattern removes it.
    DialogOk => ("확인", "OK"),
    DeletePattern => ("패턴 삭제", "Delete Pattern"),
    /// The Suppress selector: `{0}` how many elements are suppressed.
    SuppressElements => ("억제: 요소 {0}개", "Suppress: {0} elements"),
    DialogDone => ("완료", "Done"),
    /// Dialog: the edit was refused. `{0}` why.
    DialogRefused => ("만들 수 없습니다: {0}", "Cannot create: {0}"),
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
