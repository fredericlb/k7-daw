// K7ClientTests.swift — the native client against a stubbed backend: the session and hello come first, the file
// service is called with the hello's ticket, and a 401 asks for a fresh ticket once.

import XCTest
@testable import k7

/// Answers every request from `StubProtocol.answer`, and keeps what was asked.
final class StubProtocol: URLProtocol {
    nonisolated(unsafe) static var answer: (URLRequest) -> (Int, String) = { _ in (404, "{}") }
    nonisolated(unsafe) static var asked: [URLRequest] = []

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.asked.append(request)
        let (status, body) = Self.answer(request)
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: nil)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

final class K7ClientTests: XCTestCase {
    private var client: K7Client!

    override func setUp() {
        StubProtocol.asked = []
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubProtocol.self]
        client = K7Client(origin: URL(string: "http://127.0.0.1:1")!, session: URLSession(configuration: config))
    }

    private static let list = #"{"workspaces":[{"name":"Songs","isSingleProject":false,"projects":[{"path":"a.k7","app":"k7","surfaces":["timeline"],"editedAt":1700000000000}]}]}"#

    func testHelloThenWorkspacesWithTheTicket() async throws {
        StubProtocol.answer = { r in
            switch r.url!.path {
            case "/auth/me": return (200, #"{"user":{"email":"dev@example.com"}}"#)
            case "/api/files-hub": return (200, #"{"path":"/api/files","ticket":"T1"}"#)
            case "/api/files/workspaces/projects": return (200, Self.list)
            default: return (404, "{}")
            }
        }
        let list = try await client.workspaces()
        XCTAssertEqual(list.map(\.name), ["Songs"])
        XCTAssertEqual(list.first?.projects.first?.surfaces, ["timeline"])
        let files = try XCTUnwrap(StubProtocol.asked.last)
        XCTAssertEqual(files.value(forHTTPHeaderField: "x-studio-engine-token"), "T1")
        XCTAssertEqual(files.value(forHTTPHeaderField: "x-studio-user"), "dev@example.com")
        XCTAssertEqual(StubProtocol.asked.map { $0.url!.path }, ["/auth/me", "/api/files-hub", "/api/files/workspaces/projects"])
    }

    func testAStaleTicketIsAskedForAgainOnce() async throws {
        var hellos = 0
        StubProtocol.answer = { r in
            switch r.url!.path {
            case "/auth/me": return (200, #"{"user":{"sub":"local"}}"#)
            case "/api/files-hub":
                hellos += 1
                return (200, #"{"path":"/api/files","ticket":"T\#(hellos)"}"#)
            default:
                return r.value(forHTTPHeaderField: "x-studio-engine-token") == "T2" ? (200, Self.list) : (401, #"{"error":"stale"}"#)
            }
        }
        let list = try await client.workspaces()
        XCTAssertEqual(list.count, 1)
        XCTAssertEqual(hellos, 2)
    }

    func testARefusalKeepsTheBackendsSentence() async {
        StubProtocol.answer = { r in
            r.url!.path == "/api/files-hub" ? (200, #"{"path":"/api/files","ticket":"T"}"#)
                : r.url!.path == "/auth/me" ? (200, "{}") : (409, #"{"error":"exists already"}"#)
        }
        do {
            try await client.createWorkspace("Songs")
            XCTFail("a refusal must throw")
        } catch {
            XCTAssertEqual(error as? K7Error, K7Error(status: 409, message: "exists already"))
        }
    }

    func testCreateProjectNamesTheWorkspaceInItsHeader() async throws {
        StubProtocol.answer = { r in
            switch r.url!.path {
            case "/auth/me": return (200, "{}")
            case "/api/files-hub": return (200, #"{"path":"/api/files","ticket":"T"}"#)
            default: return (200, #"{"project":"b.k7","missing":["synth"]}"#)
            }
        }
        let made = try await client.createProject(workspace: "My songs", folder: "b.k7", shape: "mix")
        XCTAssertEqual(made.project, "b.k7")
        XCTAssertEqual(made.missing, ["synth"])
        let last = try XCTUnwrap(StubProtocol.asked.last)
        XCTAssertEqual(last.value(forHTTPHeaderField: "x-studio-workspace"), "My songs")
        XCTAssertEqual(last.httpMethod, "POST")
    }
}
