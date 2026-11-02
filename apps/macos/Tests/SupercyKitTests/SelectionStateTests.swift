import CoreGraphics
import XCTest

final class SelectionStateTests: XCTestCase {
    private let display = CGRect(x: 0, y: 0, width: 1440, height: 900)
    private let editor = CaptureWindow(id: 11, frame: CGRect(x: 100, y: 100, width: 600, height: 400))
    private let offEdge = CaptureWindow(id: 12, frame: CGRect(x: 1200, y: 700, width: 600, height: 400))

    private func fresh() -> SelectionState {
        SelectionState(bounds: display, windows: [editor, offEdge])
    }

    private func point(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: x, y: y) }

    func testHoveringHighlightsTheWindowOrTheWholeDisplay() {
        let state = fresh()

        XCTAssertEqual(state.phase, .choosing)
        XCTAssertEqual(state.hovering(at: point(200, 200)).highlight, editor.frame)
        XCTAssertEqual(state.hovering(at: point(1300, 800)).highlight, CGRect(x: 1200, y: 700, width: 240, height: 200),
                       "cut to the display")
        XCTAssertEqual(state.hovering(at: point(50, 850)).highlight, display)
    }

    func testClickingAWindowSelectsItAndRemembersWhichOne() {
        let state = fresh().hovering(at: point(200, 200)).pressed(at: point(200, 200)).released(at: point(201, 200))

        XCTAssertEqual(state.phase, .selected)
        XCTAssertEqual(state.selection, editor.frame)
        XCTAssertEqual(state.selectedWindow, editor)
        XCTAssertTrue(state.showsActions)
    }

    func testClickingTheDesktopSelectsTheWholeDisplay() {
        let state = fresh().pressed(at: point(50, 850)).released(at: point(50, 850))

        XCTAssertEqual(state.selection, display)
        XCTAssertNil(state.selectedWindow)
    }

    func testDraggingSelectsARegionOnWholePoints() {
        let drawing = fresh().pressed(at: point(10.2, 20)).dragged(to: point(110.7, 80))

        XCTAssertEqual(drawing.phase, .drawing(anchor: point(10.2, 20)))
        XCTAssertEqual(drawing.selection, CGRect(x: 10, y: 20, width: 101, height: 60))
        XCTAssertFalse(drawing.showsActions, "no action bar while dragging")

        let done = drawing.released(at: point(110.7, 80))
        XCTAssertEqual(done.phase, .selected)
        XCTAssertEqual(done.selection, CGRect(x: 10, y: 20, width: 101, height: 60))
        XCTAssertNil(done.selectedWindow)
    }

    func testATinyDragIsAClick() {
        let state = fresh().pressed(at: point(200, 200)).dragged(to: point(202, 201)).released(at: point(202, 201))

        XCTAssertEqual(state.selection, editor.frame)
    }

    func testHandlesResizeAndTheInsideMovesAndAWindowSelectionBecomesARegion() {
        let selected = fresh().pressed(at: point(200, 200)).released(at: point(200, 200))

        let resized = selected.pressed(at: point(700, 500)).dragged(to: point(720, 530)).released(at: point(720, 530))
        XCTAssertEqual(resized.selection, CGRect(x: 100, y: 100, width: 620, height: 430))
        XCTAssertNil(resized.selectedWindow)

        let moved = resized.pressed(at: point(300, 300)).dragged(to: point(290, 280)).released(at: point(290, 280))
        XCTAssertEqual(moved.selection, CGRect(x: 90, y: 80, width: 620, height: 430))
    }

    func testPressingOutsideTheSelectionStartsANewOne() {
        let selected = fresh().pressed(at: point(10, 10)).dragged(to: point(50, 50)).released(at: point(50, 50))

        let next = selected.pressed(at: point(800, 800)).dragged(to: point(900, 850)).released(at: point(900, 850))

        XCTAssertEqual(next.selection, CGRect(x: 800, y: 800, width: 100, height: 50))
    }

    func testArrowKeysOnlyActOnAFinishedSelection() {
        let selected = fresh().pressed(at: point(10, 10)).dragged(to: point(50, 50)).released(at: point(50, 50))

        XCTAssertEqual(selected.nudged(.right, resize: false, large: false).selection, CGRect(x: 11, y: 10, width: 40, height: 40))
        XCTAssertEqual(selected.nudged(.down, resize: false, large: true).selection, CGRect(x: 10, y: 20, width: 40, height: 40))
        XCTAssertEqual(selected.nudged(.right, resize: true, large: false).selection, CGRect(x: 10, y: 10, width: 41, height: 40))
        XCTAssertEqual(fresh().nudged(.right, resize: false, large: false), fresh())
    }

    func testSelectAllPresetAndClear() {
        XCTAssertEqual(fresh().selectingAll().selection, display)

        let repeated = SelectionState(bounds: display, windows: [], selection: CGRect(x: 5, y: 5, width: 10, height: 10))
        XCTAssertEqual(repeated.phase, .selected)
        XCTAssertTrue(repeated.showsActions)

        let offDisplay = SelectionState(bounds: display, windows: [], selection: CGRect(x: 5000, y: 5, width: 10, height: 10))
        XCTAssertEqual(offDisplay.phase, .choosing, "a last region that no longer fits is dropped")

        XCTAssertEqual(repeated.cleared().phase, .choosing)
        XCTAssertNil(repeated.cleared().selection)
    }

    func testHandleLookupNeedsAFinishedSelection() {
        let drawing = fresh().pressed(at: point(10, 10)).dragged(to: point(50, 50))

        XCTAssertNil(drawing.handle(at: point(30, 30)))
        XCTAssertEqual(drawing.released(at: point(50, 50)).handle(at: point(30, 30)), .inside)
    }
}
