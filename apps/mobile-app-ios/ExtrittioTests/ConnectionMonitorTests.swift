import Testing
@testable import Extrittio

@Suite("ConnectionMonitor")
struct ConnectionMonitorTests {

    @Test("initial state is connected")
    @MainActor
    func initialState() {
        let monitor = ConnectionMonitor(serverAddressProvider: MockServerAddressProvider())
        #expect(monitor.state == .connected)
    }

    @Test("connection states exist")
    func connectionStates() {
        let states: [ConnectionState] = [.connected, .reconnecting, .disconnected]
        #expect(states.count == 3)
    }
}

struct MockServerAddressProvider: ServerAddressProvider {
    var serverAddress: String { "192.168.1.1" }
}
