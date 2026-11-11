// THE WIRE SCHEMA CONVERTER (round 14, Arm A — the ONE generic Swift
// change).
//
// This file lives in the LIBRARY target, not the `brain-afm` executable,
// for the same reason `GroundedAnswer` does: the converter is shared
// vocabulary — the bridge child builds from it, and the protocol pins
// round-trip it — and a test target cannot import an executable.
//
// WHY THIS EXISTS. `@Generable` is a COMPILE-TIME macro: the AFM tier's
// typed chat contract (`GroundedAnswer`) is fixed at build. The kernel's
// GDL loop needs a DIFFERENT shape per artifact phase, delivered at
// RUNTIME over the bridge wire — and FoundationModels has a first-class
// runtime API for exactly that: `DynamicGenerationSchema` (properties,
// descriptions, anyOf, arrays) wrapped in a `GenerationSchema`, and
// `LanguageModelSession.respond(to:schema:)` returns the settled
// `GeneratedContent`. ONE converter here, then every artifact schema
// flows over the wire with zero per-artifact Swift changes.
//
// THE DIALECT IS SMALL ON PURPOSE — exactly what the kernel's Rust
// builder (`intake_artifact_schema_json` in `src/workflow/gdl.rs`)
// emits, and nothing else:
//   object   {"type":"object","description":…,"properties":{…},"required":[…]}
//   string   {"type":"string","description":…}
//   integer  {"type":"integer"} · number {"type":"number"} · boolean {"type":"boolean"}
//   array    {"type":"array","items":{…},"minItems":N,"maxItems":M}
//   anyOf    {"anyOf":[…],"description":…}
// Anything outside the dialect throws a NAMED error — the caller refuses
// the turn `bad_request` BEFORE any model work. Fail-closed, never
// silently unguided: a schema the child cannot honor must refuse, not
// degrade to free-form generation the kernel's gates then judge blind.
//
// GUIDED GENERATION GUARANTEES SHAPE, NEVER TRUTH (the LCD-bias caution):
// the schema makes the artifact WELL-FORMED; the kernel's gates still
// judge content — the honesty law is unchanged by transport.

import Foundation
#if canImport(FoundationModels)
import FoundationModels

@available(macOS 26.0, *)
public enum WireSchemaConverter {
    /// The named failure taxonomy: each case maps to the child's
    /// `bad_request` refusal with this text, at parse time.
    public enum ConverterError: Error, CustomStringConvertible {
        case notAnObject(String)
        case unknownSchemaDialect(String)
        case badProperty(String)

        public var description: String {
            switch self {
            case .notAnObject(let detail):
                return "schema must decode as a JSON object: \(detail)"
            case .unknownSchemaDialect(let detail):
                return "unknown schema dialect: \(detail)"
            case .badProperty(let detail):
                return "schema property is malformed: \(detail)"
            }
        }
    }

    /// Parse the wire document (a JSON-schema string) and build the
    /// runtime schema the session generates against.
    public static func generationSchema(from wire: String) throws -> GenerationSchema {
        let root = try dynamicSchema(fromWire: wire, name: "root")
        return try GenerationSchema(root: root, dependencies: [])
    }

    /// One node of the wire document → one runtime schema node. Static and
    /// pure: same string in, same schema out, no model touched.
    static func dynamicSchema(fromWire wire: String, name: String) throws -> DynamicGenerationSchema {
        guard let data = wire.data(using: .utf8),
              let object = (try? JSONSerialization.jsonObject(with: data))
                as? [String: Any]
        else {
            throw ConverterError.notAnObject(String(wire.prefix(80)))
        }
        return try dynamicSchema(from: object, name: name)
    }

    static func dynamicSchema(from object: [String: Any], name: String) throws -> DynamicGenerationSchema {
        let description = object["description"] as? String
        // anyOf: the union shape (the decomposition-style alternatives).
        if let choices = object["anyOf"] as? [[String: Any]] {
            var branches: [DynamicGenerationSchema] = []
            for (index, choice) in choices.enumerated() {
                branches.append(
                    try dynamicSchema(from: choice, name: "\(name).anyOf\(index)")
                )
            }
            return DynamicGenerationSchema(
                name: name, description: description, anyOf: branches
            )
        }
        switch object["type"] as? String {
        case "string":
            return DynamicGenerationSchema(type: String.self)
        case "integer":
            return DynamicGenerationSchema(type: Int.self)
        case "number":
            return DynamicGenerationSchema(type: Double.self)
        case "boolean":
            return DynamicGenerationSchema(type: Bool.self)
        case "array":
            guard let items = object["items"] as? [String: Any] else {
                throw ConverterError.unknownSchemaDialect("array without items")
            }
            let itemSchema = try dynamicSchema(from: items, name: "\(name).items")
            return DynamicGenerationSchema(
                arrayOf: itemSchema,
                minimumElements: object["minItems"] as? Int,
                maximumElements: object["maxItems"] as? Int
            )
        case "object":
            guard let properties = object["properties"] as? [String: Any] else {
                throw ConverterError.unknownSchemaDialect("object without properties")
            }
            let required = Set((object["required"] as? [String]) ?? [])
            var props: [DynamicGenerationSchema.Property] = []
            for (key, raw) in properties {
                guard let shape = raw as? [String: Any] else {
                    throw ConverterError.badProperty(key)
                }
                props.append(
                    DynamicGenerationSchema.Property(
                        name: key,
                        description: shape["description"] as? String,
                        schema: try dynamicSchema(from: shape, name: key),
                        isOptional: !required.contains(key)
                    )
                )
            }
            return DynamicGenerationSchema(
                name: name, description: description, properties: props
            )
        case let other:
            throw ConverterError.unknownSchemaDialect(other ?? "no type and no anyOf")
        }
    }
}
#endif
