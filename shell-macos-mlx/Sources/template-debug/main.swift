// CG-10 diagnostic: bisects the Qwen3.5 turn-2 raise ("No user query found in
// messages.") through the REAL rendering path. Ground truth from python jinja2:
// the template renders the exact turn-2 list FINE — so the divergence is in the
// Swift engine or the message encoding. This tool finds which.
import Foundation
import Tokenizers
import Jinja

import OrderedCollections
func vmsg(_ role: String, _ content: String) -> [String: Value] {
    ["role": .string(role), "content": .string(content)]
}
// dict messages for the swift-transformers path (Message = [String: Any])
let user1  = ["role": "user", "content": "Remember this number for later: 184/97. Reply only 'ok'."]
let a1Plain = ["role": "assistant", "content": "ok"]
let a1Think = ["role": "assistant", "content": "<think>\nI will remember 184/97.\n</think>\n\nok"]
let user2  = ["role": "user", "content": "What was the number I asked you to remember? Numbers only."]
// typed messages for the direct Jinja 3 path
let vUser1 = vmsg("user", "Remember this number for later: 184/97. Reply only 'ok'.")
let vA1    = vmsg("assistant", "ok")
let vUser2 = vmsg("user", "What was the number I asked you to remember? Numbers only.")

func arm(_ name: String, _ body: () throws -> String) {
    do { let out = try body(); print("\(name): OK — tail: ...\(out.suffix(60).replacingOccurrences(of: "\n", with: "\\n"))") }
    catch { print("\(name): THREW — \(error)") }
}

// ── ARM A: the engine DIRECTLY, dict context, the 3.5 template file ────────
let src = (try? String(contentsOfFile: "/tmp/qwen35_quant.jinja", encoding: .utf8)) ?? ""
if src.isEmpty {
    print("ARM A: skipped (template file missing)")
} else {
    arm("A1 engine-direct [u1,a1,u2]") {
        try Template(src).render(["messages": Value.array([.object(vUser1), .object(vA1), .object(vUser2)]), "add_generation_prompt": .boolean(true)])
    }
    arm("A2 engine-direct [u1,a1<think>]") {
        try Template(src).render(["messages": Value.array([.object(vUser1), .object(vmsg("assistant", "<think>\nI will remember 184/97.\n</think>\n\nok")), .object(vUser2)]), "add_generation_prompt": .boolean(true)])
    }
    arm("A3 engine-direct [u1,a1] (no trailing user)") {
        try Template(src).render(["messages": Value.array([.object(vUser1), .object(vA1)]), "add_generation_prompt": .boolean(true)])
    }
}

// ── ARM B: the FULL path — AutoTokenizer from the OFFICIAL repo ────────────
let tok = try await AutoTokenizer.from(pretrained: "Qwen/Qwen3.5-2B")
func cc(_ dict: [String: String]) -> Message { dict }
arm("B1 applyChatTemplate [u1]") {
    let t = try tok.applyChatTemplate(messages: [cc(user1)])
    return "(tokens: \(t.count))"
}
arm("B2 applyChatTemplate [u1,a1]") {
    let t = try tok.applyChatTemplate(messages: [cc(user1), cc(a1Plain)])
    return "(tokens: \(t.count))"
}
arm("B3 applyChatTemplate [u1,a1,u2]") {
    let t = try tok.applyChatTemplate(messages: [cc(user1), cc(a1Plain), cc(user2)])
    return "(tokens: \(t.count))"
}
arm("B4 applyChatTemplate [u1,a1<think>,u2]") {
    let t = try tok.applyChatTemplate(messages: [cc(user1), cc(a1Think), cc(user2)])
    return "(tokens: \(t.count))"
}

// ── ARM C: fragment matrix — which construct kills the scan? ───────────────
let MACRO = """
{%- macro render_content(content, do_vision_count, is_system_content=false) %}
    {%- if content is string %}
        {{- content }}
    {%- elif content is iterable and content is not mapping %}
        {%- for item in content %}{{- '?' }}{%- endfor %}
    {%- elif content is none or content is undefined %}
        {{- '' }}
    {%- else %}
        {{- raise_exception('Unexpected content type.') }}
    {%- endif %}
{%- endmacro %}
"""
let msgs = [user1, a1Plain]

func frag(_ name: String, _ body: String) {
    do {
        let vmsgs: [Value] = msgs.map { m in
            var od: OrderedDictionary<ObjectKey, Value> = [:]
            for (k, v) in m { od[.string(k)] = .string(v) }
            return .object(od)
        }
        let out = try Template(MACRO + body).render([
            "messages": Value.array(vmsgs), "add_generation_prompt": .boolean(true),
        ])
        print("C \(name): OK — \(out.suffix(40).replacingOccurrences(of: "\n", with: "\\n"))")
    } catch { print("C \(name): THREW — \(error)") }
}

frag("C1 as-is ([::-1] + macro)", """
{%- set ns = namespace(multi_step_tool=true, last_query_index=messages|length - 1) %}
{%- for message in messages[::-1] %}
    {%- set index = (messages|length - 1) - loop.index0 %}
    {%- if ns.multi_step_tool and message.role == "user" %}
        {%- set content = render_content(message.content, false)|trim %}
        {%- if not(content.startswith('<tool_response>') and content.endswith('</tool_response>')) %}
            {%- set ns.multi_step_tool = false %}
            {%- set ns.last_query_index = index %}
        {%- endif %}
    {%- endif %}
{%- endfor %}
{%- if ns.multi_step_tool %}{{- 'FLAG-STILL-TRUE' }}{%- else %}{{- 'FLAG-CLEARED idx=' ~ ns.last_query_index }}{%- endif %}
""")

frag("C2 forward loop (no [::-1])", """
{%- set ns = namespace(multi_step_tool=true, last_query_index=messages|length - 1) %}
{%- set rev = messages|reverse %}
{%- for message in rev %}
    {%- if ns.multi_step_tool and message.role == "user" %}
        {%- set content = render_content(message.content, false)|trim %}
        {%- if not(content.startswith('<tool_response>') and content.endswith('</tool_response>')) %}
            {%- set ns.multi_step_tool = false %}
        {%- endif %}
    {%- endif %}
{%- endfor %}
{%- if ns.multi_step_tool %}{{- 'FLAG-STILL-TRUE' }}{%- else %}{{- 'FLAG-CLEARED' }}{%- endif %}
""")

frag("C3 [::-1] but NO macro (direct .content)", """
{%- set ns = namespace(multi_step_tool=true) %}
{%- for message in messages[::-1] %}
    {%- if ns.multi_step_tool and message.role == "user" %}
        {%- set content = message.content %}
        {%- if not(content.startswith('<tool_response>') and content.endswith('</tool_response>')) %}
            {%- set ns.multi_step_tool = false %}
        {%- endif %}
    {%- endif %}
{%- endfor %}
{%- if ns.multi_step_tool %}{{- 'FLAG-STILL-TRUE' }}{%- else %}{{- 'FLAG-CLEARED' }}{%- endif %}
""")

frag("C4 macro call OUTSIDE any loop", """
{%- set c = render_content(user1.content, false)|trim %}
{{- 'MACRO-RETURNED: ' ~ c }}
""")

frag("C5 slice alone — what does [::-1] yield?", """
{%- set r = messages[::-1] %}
{{- 'LEN=' ~ (r|length) ~ ' FIRST-ROLE=' ~ r[0].role }}
""")

// ── ARM D: characterize the slice bug precisely ────────────────────────────
func sliceProbe(_ name: String, _ list: [[String: String]]) {
    do {
        let vmsgs: [Value] = list.map { m in
            var od: OrderedDictionary<ObjectKey, Value> = [:]
            for (k, v) in m { od[.string(k)] = .string(v) }
            return .object(od)
        }
        let out = try Template("{%- for m in messages[::-1] %}{{- m.role ~ ',' }}{%- endfor %}")
            .render(["messages": Value.array(vmsgs)])
        print("D \(name): rendered roles: \(out)")
    } catch { print("D \(name): THREW — \(error)") }
}
sliceProbe("D1 [::-1] over [u1]", [user1])
sliceProbe("D2 [::-1] over [u1,a1]", [user1, a1Plain])
sliceProbe("D3 [::-1] over [a1,u1]", [a1Plain, user1])
sliceProbe("D4 [:] slice over [u1,a1]", [user1, a1Plain])
sliceProbe("D5 [::-1] literal 3", [user1, a1Plain, user2])
do {
    let vmsgs: [Value] = [user1, a1Plain].map { m in
        var od: OrderedDictionary<ObjectKey, Value> = [:]
        for (k, v) in m { od[.string(k)] = .string(v) }
        return .object(od)
    }
    let out = try Template("{{ messages[1:2] }}").render(["messages": Value.array(vmsgs)])
    print("D6 explicit [1:2]: \(out.suffix(60))")
} catch { print("D6: THREW — \(error)") }
