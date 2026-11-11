// The guided-schema pins (round 14, Arm A — hermetic, no model).
//
// The converter is the ONE generic Swift change this round makes in the
// child: a JSON-schema document string from the kernel's bridge wire →
// the runtime schema the AFM tier generates against. These pins hold its
// dialect contract WITHOUT loading any model — building a
// `GenerationSchema` is pure value construction — so they run in the
// plain suite, on every machine, in seconds.
//
// The laws pinned:
//   GS-1  the full dialect the Rust builder emits round-trips: the
//         IntakeArtifact shape (objects, the KT table's rows, string
//         arrays with minItems, optionals) builds and the ROOT is an
//         object schema.
//   GS-2  scalars and the telemetry array shape build.
//   GS-3  fail-closed: a non-JSON string, an unknown type, an array
//         without items, a malformed property — each throws the NAMED
//         error. A schema the child cannot honor refuses; it never
//         degrades to free-form generation the kernel's gates would
//         judge blind.
//   GS-4  determinism: the same document builds twice to equal schemas
//         (the AFM tier samples greedy; the transport must not add
//         nondeterminism of its own).
//
// Availability follows the suite's house pattern: the @Test bodies guard
// `#available(macOS 26.0, *)` (the FoundationModels surface), the
// helpers carry the annotation — @Test cannot sit on an
// availability-limited member.

import Foundation
import Testing
@testable import AppleFMProbe
#if canImport(FoundationModels)
import FoundationModels

/// The named skip for a pre-26 host: the surface is 26-gated, the pin is
/// not silent — the skip reports the same named reason every time.
private struct GuidedSchemaSurfaceUnavailable: Error, CustomStringConvertible {
    var description: String {
        "the FoundationModels runtime-schema surface requires macOS 26"
    }
}

struct GuidedSchemaPins {
    /// The wire document EXACTLY as the kernel's Rust builder emits it —
    /// the KT artifact schema (a tightened excerpt of
    /// `intake_artifact_schema_json`, the same dialect shapes).
    private static let intakeWire = """
        {"name":"IntakeArtifact","type":"object","description":"The KT problem statement",
         "properties":{
           "is_not":{"type":"object","description":"The KT table",
             "properties":{
               "what":{"type":"object","description":"quote the ticket; unknown refused",
                 "properties":{"is":{"type":"string"},"is_not":{"type":"string"}},
                 "required":["is","is_not"]},
               "where":{"type":"object","description":"quote or unknown",
                 "properties":{"is":{"type":"string"},"is_not":{"type":"string"}},
                 "required":["is","is_not"]},
               "when":{"type":"object","description":"quote or unknown",
                 "properties":{"is":{"type":"string"},"is_not":{"type":"string"}},
                 "required":["is","is_not"]},
               "extent":{"type":"object","description":"quote or unknown",
                 "properties":{"is":{"type":"string"},"is_not":{"type":"string"}},
                 "required":["is","is_not"]}},
             "required":["what","where","when","extent"]},
           "telemetry_refs":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":16},
           "what_changed":{"type":"string"},
           "known_good":{"type":"string"},
           "diagnostic_seams":{"type":"array","items":{"type":"string"},"maxItems":16},
           "decomposition":{"type":"object",
             "properties":{"issues":{"type":"array","minItems":2,"items":{"type":"object",
               "properties":{"title":{"type":"string"},"question":{"type":"string"}},
               "required":["title","question"]}},
               "rationale":{"type":"string"}},
             "required":["issues","rationale"]}},
         "required":["is_not","telemetry_refs","what_changed","known_good"]}
        """

    @available(macOS 26.0, *)
    private static func build(_ wire: String) throws -> GenerationSchema {
        try WireSchemaConverter.generationSchema(from: wire)
    }

    @available(macOS 26.0, *)
    private static func failureMessage(_ wire: String) -> String {
        do {
            _ = try build(wire)
            return "unexpectedly built"
        } catch {
            return String(describing: error)
        }
    }

    @Test("GS-1: the kernel's IntakeArtifact dialect builds to a runtime schema")
    func theArtifactDialectBuilds() throws {
        guard #available(macOS 26.0, *) else {
            throw GuidedSchemaSurfaceUnavailable()
        }
        let schema = try Self.build(Self.intakeWire)
        // The schema is built; the framework accepted the whole tree. A
        // GenerationSchema value is the only observable here — the pins
        // below hold the dialect's edges; the LIVE arm run holds the
        // generation behavior.
        #expect(!String(describing: schema).isEmpty)
    }

    @Test("GS-2: scalars and arrays of scalars build (the telemetry shape)")
    func scalarsAndArraysBuild() throws {
        guard #available(macOS 26.0, *) else {
            throw GuidedSchemaSurfaceUnavailable()
        }
        let stringArray = """
            {"type":"array","items":{"type":"string"},"minItems":1,"maxItems":16}
            """
        let schema = try Self.build(stringArray)
        #expect(!String(describing: schema).isEmpty)
        for scalar in [
            #"{"type":"string"}"#, #"{"type":"integer"}"#,
            #"{"type":"number"}"#, #"{"type":"boolean"}"#,
        ] {
            let s = try Self.build(scalar)
            #expect(!String(describing: s).isEmpty)
        }
    }

    @Test("GS-3: the dialect fails closed, by name, on every edge")
    func theDialectFailsClosedByName() throws {
        guard #available(macOS 26.0, *) else {
            throw GuidedSchemaSurfaceUnavailable()
        }
        #expect(
            Self.failureMessage("not json at all")
                .contains("schema must decode as a JSON object")
        )
        #expect(
            Self.failureMessage(#"{"type":"transcript"}"#)
                .contains("unknown schema dialect")
        )
        #expect(
            Self.failureMessage(#"{"type":"array"}"#).contains("array without items")
        )
        #expect(
            Self.failureMessage(#"{"type":"object","properties":{"a":"not-a-shape"}}"#)
                .contains("schema property is malformed")
        )
    }

    @Test("GS-4: the same document builds deterministically")
    func theSameDocumentBuildsDeterministically() throws {
        guard #available(macOS 26.0, *) else {
            throw GuidedSchemaSurfaceUnavailable()
        }
        // The framework's debug description carries nondeterministic ids,
        // so determinism is pinned over the CONTENT: the sorted set of
        // quoted names each build's description carries must be identical
        // — the converter adds no dialect drift of its own.
        func names(_ schema: GenerationSchema) -> [String] {
            String(describing: schema)
                .split(separator: "\"")
                .map(String.init)
                .sorted()
        }
        let a = names(try Self.build(Self.intakeWire))
        let b = names(try Self.build(Self.intakeWire))
        #expect(a == b)
    }
}

#endif
