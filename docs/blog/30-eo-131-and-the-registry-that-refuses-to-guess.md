# EO 131 and the registry that refuses to guess

*2026-11-23. The Philippines put an AI governance framework over its national
government agencies this month, with a quarterly reporting duty. The useful
engineering question is not "are we compliant". It is whether a machine can
say, on any date, which rules governed it, and prove it. This week we shipped
the piece that answers that. It starts by refusing to answer for one of the
instruments.*

## The order, and what it asks for

Executive Order No. 131, series of 2026 was signed on **November 12** and
took effect on **November 14**. It adopts the Philippine AI Governance
Framework. It also creates the National Artificial Intelligence Council. The
Council is chaired by the Executive Secretary. The Department of Economy,
Planning and Development acts as its secretariat.

Two clauses matter to anyone who runs software inside an agency.

- **Section 5.** Agencies update the monitoring and reporting platform that
  the Department of Information and Communications Technology (DICT) runs.
  They update it every quarter. The Council then reports to the President
  within thirty days after each quarter ends.
- **Section 6.** The Framework itself is reviewed at least every two years.

The order carries no penal clause. It asks for evidence on a clock.

Read those two clauses together. A requirement falls out that no compliance
document spells out. **A quarterly report has to describe the rules as they
stood during that quarter.** It cannot describe them as they stand on the
day someone writes the report. An instrument superseded in March changes
what April's filing should have said. A Framework reviewed in year two does
not rewrite what year one required.

## What we do not have, said first

Here is the part most vendors leave out. **We do not hold the text of EO
131.** The facts above come from an analysis by a law firm, Aureada CPA Law,
published on November 14. They also come from references to the Official
Gazette. The operative document itself has not been fetched, hashed, and
filed. We tried to fetch it while writing this post. The Official Gazette
website refused the request.

Our registry says so. In its vocabulary the order is `text_fetch_owed`. That
means the instrument is known to be in force, but its text is not in hand.
Ten of the twelve instruments we track are in that state. Two are not. For
those two we hold the full text and have hashed it:

- DepEd Order No. 003, series of 2026 (basic education)
- CHED Memorandum Order No. 21, series of 2026 (higher education)

So the count today is twelve tracked, two verified, ten owed. A pack that
requires one of the ten will not activate as though the text were in hand.

We built the refusal before we built anything else. That was on purpose.
The temptation with a new regulation is to map it onto whatever you already
sell, then publish a table of green ticks. That table is worth exactly the
confidence you have in the mapping. Our mapping rests on a secondary source.
A system that cannot say "not verified" cannot be trusted when it says
"verified".

One more honesty check, because it is easy to get wrong. For DepEd Order 003
the hashed file is the **original scan**, the PDF. We also hold a text
transcription of it. The transcription is a derivative. Its own closing note
tells the reader to check it against the published PDF. The registry hashes
the original and never the derivative.

The dates have a caveat too. Both held orders say they take effect "fifteen
days after publication" and filing. Neither one gives a calendar date for
that. So the registry records the date printed on each order, the day it was
signed: February 20 for DepEd, July 21 for CHED. The receipt below labels
that `dated`. It does not say "in force from", because the order's own text
does not fix that day. A report that needs the exact effectivity date has to
find the publication date. The registry will not invent one.

## The registry

What shipped is small and deliberate. It is a table of instruments and three
commands.

```
brain policy-anchor seed   --manifest ANCHORS.json --corpus-root ph-ai-policies
brain policy-anchor verify --as-of 2026-06-30
brain policy-anchor gate   --pack gov.education
```

A manifest is a short file that lists the instruments and the hash (a
fingerprint) of each text we claim to hold. A pack is a bundle of rules and
content for one department. A gate decides whether a pack may switch on.

**`seed` reads the manifest and fills the registry.** For every instrument
the manifest claims to hold, it opens the file and hashes the bytes. It
compares that hash to the recorded one. A mismatch refuses the whole seed.
The refusal names the file, the expected hash, and the hash it measured.
Hashing at load time is the point. The registry never trusts a claim of
possession. It trusts bytes it has read.

**`verify` answers a dated question.** Given `--as-of`, it returns the
instruments in force on that date. Run against today's registry for the
middle of this year, it returns eight of the twelve. Four carry a later
date. That is the quarter-boundary answer a Section 5 filing needs. It gives
the same answer every time it is asked. The answer is only as good as the
dates the instruments state, which is why the date caveat above matters.

**`gate` is the refusal.** A pack names the instruments it depends on. The
gate decides whether the pack may switch on. It lists every blocker, each
with its own reason. Here is the real output for an education pack that was
bound to an instrument whose text we do not hold:

```
pack gate for 'gov.education' — refused
  ✓ PH-DEPED-DO-003-2026         verified  (dated 2026-02-20)  sha256:18263a9a6f426c76479b32a3c3807d919acd0d40f1b64cad8badf9c96144202b
  ✓ PH-CHED-CMO-21-2026          verified  (dated 2026-07-21)  sha256:0a4536a74b891cd2dc7f20bffa736a321ad8f9f33678e02a73874735b1bfe01f
  ✗ PH-EO-131-2026               advisory-only — anchor text not held
```

The command exits with code 2. The two held instruments pass. The order is
named. The reason is the sentence an operator needs. The pack is not blocked
because the rule was broken. It is blocked because the rule's text has not
been checked. "Advisory-only" is not a polite word for approved. It is the
opposite of approved. It is written so that nobody mistakes a deployment for
a verified one.

An unset configuration is refused too. With no bindings named, the gate does
not conclude that nothing is required. It refuses, because a gate nobody
configured has decided nothing. Failing closed includes failing closed on
ourselves. A configuration that cannot be parsed is dropped whole, with a
warning. It is never applied halfway.

## The words are the product

The registry has four states for an instrument: `verified_text_held`,
`text_fetch_owed`, `superseded`, and `retired`. It has three verdicts for a
pack: `verified`, `advisory-only`, and `refused`.

The word "compliant" appears nowhere in it. That is a decision, not an
oversight. Compliance is a legal conclusion. It belongs to the agency and its
lawyers. A memory server is not qualified to draw it. What the server can do
is state facts. This text was hashed. This instrument was in force on this
date. This pack depends on an instrument we have not read. Those facts help
the person who draws the conclusion. A green tick that pre-empts them does
not.

The same rule governs history. When an instrument is superseded, the old row
is not edited. A new row lands. The old one is stamped with the date it
stopped being in force. Both stay. Every supersede is one transaction. The
stamp, the new row, and the audit record commit together, or none of them
do.

We tested that by force. We planted a fault that fires after the new row is
written. Then we checked that the old row was left untouched. The first
version of that test split the work into two transactions. The fault left a
stamp with nothing to stamp against. The test caught it.

## What the rest of the system contributes

The registry is one piece. An agency's duties under the order's principles
(transparency, accountability, human oversight, privacy) are answered by
parts of this system that have been in the tree for months. Each one says
what it did.

- **Every answer carries its evidence.** A chat turn cites the memory it
  drew on. A deterministic check, with no model involved, then states
  whether each cited passage contains the claim. The verdict is
  `supported`, `partial`, or `unsupported`. An unsupported claim is shown as
  unsupported. It is not quietly kept. See the
  [API reference](../api.md).
- **Every write leaves a record that cannot be quietly changed.** The audit
  chain links each row to the one before it. The link is HMAC-SHA256 (a
  keyed fingerprint) over the full row, under a key that never lives in the
  database. Someone holding only the database cannot forge a link. The head
  pin catches truncated or extended chains. See the
  [security policy](../SECURITY.md).
- **Nothing enters memory without a human decision.** Agents propose. People
  approve. The approval is bound to a hash of the exact text the person saw,
  so the content cannot change between review and approval. An optional
  setting requires two different people to approve. It is off by default.
  See [human-in-the-loop](../human-in-the-loop.md).
- **Deletion produces a certificate.** A request from a data subject runs
  locate, export, purge, tombstone, certificate. The certificate records what
  was found and what was purged. See the
  [compliance posture](../compliance.md).
- **Untrusted content is screened on the way in.** The write core screens
  every ingest for prompt injection and quarantines or rejects it. On the
  way out, a read seam (one shared function that every text field passes
  through) redacts personal data. See the
  [threat model](../THREAT_MODEL.md) and the
  [proof map](../trust/proof-map.md), which ties each claim to the test that
  holds it.

None of that was built for EO 131. It was built because a memory store that
an agent reads on every turn has to be careful about what it holds. The order
asks agencies to be careful in the same directions. That is why the overlap
is large. It is also why the gaps are specific and not general.

## What is owed

The registry makes the gaps listable. That is an improvement over a
narrative. Today it holds these.

- **Ten instruments with no text held.** EO 131 is one. Each one joins the
  held set the same way. Someone drops the official document into the
  corpus. The hash is measured. The registry stops refusing. One hash at a
  time, never by loosening the check.
- **The quarterly export.** The registry can answer the as-of question. The
  machine-readable report an agency files with the Council is the next
  piece. It is scoped and not built.
- **The wire surface.** The three commands are a command-line tool. The same
  reads as an HTTP endpoint. A pack that refuses at load time, rather than
  at a command, follows in the next round.

We have also mapped every clause of the order onto a mechanism, with a
percentage attached. That is our own judgment, made in private. We are not
publishing it as a claim. A number like that invites the reader to stop
checking. The registry exists so they do not have to take our word.

## The date that matters

EO 131 took effect on November 14. The first quarterly cycle under it closes
at the end of December. The Council's report follows within thirty days. An
agency that wants to file something true in January needs, in December, a
record of which instruments governed it and what its systems actually did.

That record is either something a system kept, or something someone
rebuilds from memory in January. We would rather it be the first.

---

*The registry ships in [v1.29.48](../CHANGELOG.md). The commands are in the
[CLI reference](../cli-reference.md). EO 131 facts come from the Aureada CPA
Law analysis of November 14, 2026 and from Official Gazette references. The
order's text is `text_fetch_owed` in the registry. This post does not claim
otherwise.*
