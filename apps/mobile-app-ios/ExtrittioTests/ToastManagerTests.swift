import Testing
@testable import Extrittio

@Suite("ToastManager")
struct ToastManagerTests {

    @Test("show adds toast to queue")
    @MainActor
    func showAddsToast() {
        let manager = ToastManager()
        manager.show(.success("Saved"))
        #expect(manager.currentToast != nil)
        #expect(manager.currentToast?.message == "Saved")
    }

    @Test("toast types have correct SF symbols")
    func toastTypeSymbols() {
        #expect(ToastType.success("").icon == "checkmark.circle.fill")
        #expect(ToastType.error("").icon == "xmark.circle.fill")
        #expect(ToastType.info("").icon == "info.circle.fill")
        #expect(ToastType.copied.icon == "doc.on.doc.fill")
        #expect(ToastType.warning("").icon == "exclamationmark.triangle.fill")
    }

    @Test("toast types have correct durations")
    func toastTypeDurations() {
        #expect(ToastType.success("").duration == 2.0)
        #expect(ToastType.error("").duration == 3.0)
        #expect(ToastType.info("").duration == 2.0)
        #expect(ToastType.copied.duration == 1.5)
        #expect(ToastType.warning("").duration == 3.0)
    }
}
