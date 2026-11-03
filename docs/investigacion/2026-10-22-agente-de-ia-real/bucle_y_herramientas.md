# Agent loop and tool-interface design for a general PixPin assistant

Scope: how to make an LLM agent (Claude Haiku/Sonnet, DeepSeek, small local model) work out which of ~40 custom tools to combine, instead of following a routing list in the system prompt. Sources fetched 2026-10-22. Dates are given per source; anything before 2024 is flagged.

## 1. What Anthropic (Building effective agents, Agent SDK, Claude Code, think tool) recommends for the loop, planning and the ACI

### Takeaway
Anthropic's model of an agent is a plain loop: the LLM calls tools, reads ground-truth feedback from the environment, and repeats until it is done or hits a stop condition. The leverage is in tool design (the "ACI"), not in prompt routing. The Agent SDK spells the loop out as gather context, take action, verify, repeat. Planning should live in a visible artifact (a todo list or notes). The "think" tool is now mostly replaced by extended or interleaved thinking.

### Cited Findings
- Definitions: "Workflows are systems where LLMs and tools are orchestrated through predefined code paths." Agents are systems where "LLMs dynamically direct their own processes and tool usage, maintaining control over how they accomplish tasks." A routing list in the prompt is, in effect, a workflow pretending to be an agent. — [Anthropic, Building effective agents, 19 Dec 2024](https://www.anthropic.com/engineering/building-effective-agents)
- Agents are "just LLMs using tools based on environmental feedback in a loop". At each step they must "gain 'ground truth' from the environment" (tool results), with stopping conditions such as a maximum number of iterations, and they can pause for human feedback when blocked. — [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)
- ACI, from Appendix 2:
  - Invest as much effort in the agent-computer interface as in a human-computer interface.
  - Put example usage, edge cases, input requirements and clear boundaries against similar tools in each definition.
  - Write descriptions like a docstring for a junior developer.
  - Pick formats close to text found online, and avoid overhead such as counting lines or escaping code inside JSON strings.
  - "Poka-yoke your tools": change the arguments so they are harder to get wrong. Their example: relative paths caused errors, "we changed the tool to always require absolute filepaths—and we found that the model used this method flawlessly."
  - On SWE-bench the team spent more time optimizing tools than the overall prompt.
  — [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)
- The post (now revised) also suggests routing simple queries to a cheaper model (Claude Haiku 4.5) and complex ones to a stronger one (Sonnet 4.5). That is routing by model, not by tool. — [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)
- Agent SDK loop: "gather context -> take action -> verify work -> repeat."
  - Gather context: agentic search with grep/tail first, semantic search only if needed. Subagents with isolated context. Compaction near the context limit.
  - Take action: tools are "the main actions Claude considers", so they must be context-efficient. Code is "precise, composable, and infinitely reusable".
  - Verify, ranked: rules or linting are best; visual feedback (screenshots or renders) to check layout and styling; LLM-as-judge is "generally not a very robust method".
  - Improving the agent: when it fails, ask whether it has the right tools; when it keeps failing, add formal rules to tool calls; build a representative test set.
  — [Claude blog, Building agents with the Claude Agent SDK, 29 Sep 2025](https://claude.com/blog/building-agents-with-the-claude-agent-sdk)
- Context engineering (system prompts at the "right altitude"): avoid "engineers hardcoding complex, brittle logic in their prompts to elicit exact agentic behavior", and also avoid vague prompts. Other points from the same post:
  - Start with a minimal prompt on the best model, then add instructions only for observed failures.
  - Use "a few diverse, canonical examples" rather than "a laundry list of edge cases".
  - Curate "a minimal viable set of tools". If "a human engineer can't definitively say which tool should be used in a given situation", the agent won't do better.
  - Load context just in time through lightweight identifiers.
  - Long work relies on compaction, clearing stale tool results, structured note-taking (todo lists, NOTES.md) and subagents that return 1,000–2,000-token summaries.
  — [Anthropic, Effective context engineering for AI agents, 29 Sep 2025](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
- Think tool (one required `thought` string; "will not obtain new information or change the database"):
  - τ-bench airline: pass^1 went from 0.332 (baseline) to 0.584 with think tool plus an optimized prompt, against 0.404 for the think tool alone and 0.412 for extended thinking. The article's prose says 0.370 to 0.570, a "54% relative improvement", which disagrees with its own table.
  - τ-bench retail: think tool with no prompt scored 0.812 against a 0.783 baseline.
  - SWE-bench: +1.6% on average (p < .001).
  - Best uses: analyzing tool output before acting, policy-heavy environments, sequential decisions. Not worth it for parallel or simple calls.
  - Update of 15 Dec 2025: extended thinking now usually works better, "we recommend using that feature instead of a dedicated think tool in most cases."
  — [Anthropic, The "think" tool, 20 Mar 2025](https://www.anthropic.com/engineering/claude-think-tool)
- Claude Code harness, from third-party reverse-engineering (not primary documentation):
  - A single-threaded master loop: while the model's reply contains tool calls, run them, append the results, and repeat; stop when the reply is plain text.
  - Planning is made explicit with a TodoWrite list of items marked pending, in progress or done, which reduces drift.
  - A subagent tool keeps exploration out of the main context.
  — [PromptLayer, Claude Code behind the scenes of the master agent loop](https://blog.promptlayer.com/claude-code-behind-the-scenes-of-the-master-agent-loop/); [claudecn agent-loop tutorial](https://claudecn.com/docs/claude-code/advanced/agent-loop/). Treat as unverified.

### Inferences
- PixPin's fix is not a longer routing list. It is:
  1. A minimal principles prompt: what the app is, what objects exist, "inspect state, act, verify, then answer", and "if no single tool fits, combine read and edit tools".
  2. Tools whose descriptions carry the routing knowledge: what each does, when to use it, and when not to.
  3. Loop feedback: return the new state or ids, and optionally a render or screenshot for visual verification.
- Example: for "translate this figure to Chinese", a general agent should call get-selection (text and ids), translate the text itself, then call a generic update-element-text with those ids. That only works if such read and edit primitives exist and are described in general terms.
- A TodoWrite-style "plan" tool is cheap to add and helps on multi-step canvas jobs. With Haiku 4.5 and Sonnet, use extended or interleaved thinking rather than a separate think tool. A think tool may still help models without native interleaved thinking; that is an untested assumption.
- A screenshot or render of the canvas after edits is the canvas equivalent of "visual feedback" verification.

### Gaps
- I could not open the anthropic-cookbook "agents" patterns repo in this session, so its specifics are not cited here.
- The Claude Code internals cited above come from third-party analyses. No primary Anthropic document on TodoWrite's prompt text was found.

## 2. SWE-agent ACI paper: tool design, feedback format, guardrails, measured effects

### Takeaway
Interface design alone moved GPT-4 Turbo from 11% to 18% on SWE-bench Lite. The pieces that mattered were an edit command guarded by a linter, search results summarized in one shot, a bounded 100-line view, and pruned history. Every removal cost 1.7 to 7.7 points.

### Cited Findings
- Full SWE-agent resolves 18.0% of SWE-bench Lite (GPT-4 Turbo), against 11.0% for shell-only with no ACI. The paper's intro claims "10.7 percentage points more", which doesn't match its table (7.0 points). — [Yang et al., SWE-agent, arXiv 2405.15793 (2024)](https://arxiv.org/html/2405.15793)
- Ablations, as % resolved, all from the same paper:

  | Component | Variant | % resolved | Change vs 18.0 |
  |---|---|---|---|
  | Edit | No linting guardrail | 15.0 | −3.0 |
  | Edit | No edit command | 10.3 | −7.7 |
  | Search | Iterative instead of summarized | 12.0 | −6.0 |
  | Search | No search | 15.7 | −2.3 |
  | File view | 30 lines | 14.3 | −3.7 |
  | File view | Full file | 12.7 | −5.3 |
  | History | Full history instead of last 5 observations | 15.0 | −3.0 |
  | History | No demonstration | 16.3 | −1.7 |

  Iterative search did worse than no search at all.
  — [SWE-agent](https://arxiv.org/html/2405.15793)
- Four design principles:
  1. Actions simple and easy to understand.
  2. Actions compact and efficient.
  3. Feedback informative but concise.
  4. Guardrails that limit error propagation and speed recovery.
  — [SWE-agent](https://arxiv.org/html/2405.15793)
- Headline numbers: 12.5% pass@1 on full SWE-bench and 87.7% on HumanEvalFix. — [SWE-agent abstract](https://arxiv.org/abs/2405.15793)

### Inferences
- PixPin analogues:
  - Edit tools validate and reject bad edits (unknown id, empty text, out-of-canvas coordinates) with an actionable message instead of applying them. This is the linter guardrail.
  - "Find elements" returns a compact, capped list, not the full scene JSON. This is summarized search and the bounded view.
  - Old tool outputs, such as full scene dumps, are dropped from history.
- Showing too much (the full file) hurt as much as showing too little. For a canvas, return a summary such as id, type, short text and bbox, and paginate.

### Gaps
- I didn't capture the paper's exact empty-output messages ("Your command ran successfully and did not produce any output"). They are in the unread second half of the HTML.

## 3. OpenHands, smolagents/CodeAct, LangGraph plan-and-execute/ReWOO/LLMCompiler, Cline, Aider

### Takeaway
Two proven directions:
- Code as the action language (CodeAct, smolagents, OpenHands, Anthropic's Programmatic Tool Calling). It lets the model compose tools in one step, with up to 20% higher success, fewer round trips and fewer tokens.
- Split planning from execution (plan-and-execute, ReWOO, LLMCompiler, Aider architect/editor). A strong model plans and a cheap model executes.

Both reduce reliance on the model picking exactly one tool per turn.

### Cited Findings
- CodeAct (executable Python as actions) "outperforms widely used alternatives (up to 20% higher success rate)" across 17 LLMs on API-Bank and a new benchmark. Llama2 and Mistral were fine-tuned on 7k CodeActInstruct interactions. — [Wang et al., Executable Code Actions Elicit Better LLM Agents, arXiv 2402.01030 (2024)](https://arxiv.org/abs/2402.01030)
- smolagents argues that code beats JSON for composability (nesting, reusable functions), object management, generality, and presence in training data. HF's own benchmark concludes "code works better" and says open models can match closed ones. The text gives no numeric scores, and I could not confirm the widely repeated "30% fewer steps" figure there. — [Hugging Face, Introducing smolagents, 31 Dec 2024](https://huggingface.co/blog/smolagents)
- Anthropic Programmatic Tool Calling (the model writes code that calls tools):
  - Average tokens fell from 43,588 to 27,297 (−37%).
  - Knowledge retrieval accuracy rose from 25.6% to 28.5%, and GIA from 46.5% to 51.2%.
  - Running 20+ tool calls in one code block removes 19+ inference passes.
  - Recommended for three or more dependent calls or large data.
  — [Anthropic, Advanced tool use, 24 Nov 2025](https://www.anthropic.com/engineering/advanced-tool-use)
- OpenHands is a platform where agents write code, use a shell and browse in a sandbox, evaluated on 15 benchmarks including SWE-bench and WebArena (ICLR 2025, MIT license). Its default agent is CodeAct-based, but I only saw the abstract, so the architecture details are not verified here. — [Wang et al., OpenHands, arXiv 2407.16741](https://arxiv.org/abs/2407.16741)
- LangGraph planning patterns (flag: Feb 2024):
  - Plan-and-execute: a planner writes a multi-step plan, executors run it, and the planner replans. It is faster and cheaper because the large model isn't consulted after every action and sub-tasks can use lighter models, but tool calls stay serial.
  - ReWOO: the plan uses variables (#E1, #E2) so later steps consume earlier results without an LLM call per step.
  - LLMCompiler: the plan is a DAG executed in parallel, with a claimed "3.6x" speedup.
  — [LangChain, Planning agents, 13 Feb 2024](https://www.langchain.com/blog/planning-agents)
- Aider architect/editor (Sep 2024): one model reasons and describes the change, and a second turns it into formatted edits.
  - Best pairs: o1-preview with DeepSeek or o1-mini scored 85.0%, against 79.7% for o1-preview alone.
  - Self-pairing also helps: Claude 3.5 Sonnet went from 77.4% to 80.5%, GPT-4o from 71.4% to 75.2%, and GPT-4o-mini from 55.6% to 60.2%.
  - DeepSeek was a strong editor.
  — [Aider, Separating code reasoning and editing, 26 Sep 2024](https://aider.chat/2024/09/26/architect.html)
- Aider picks the edit format per model: whole, diff (search/replace), udiff (which reduced GPT-4 Turbo "laziness"), or diff-fenced for Gemini. Output format must fit the model. — [Aider, Edit formats](https://aider.chat/docs/more/edit-formats.html)

### Inferences
- For PixPin, a middle path is a single "batch/script" tool. It takes a short list of operations that reference earlier results, ReWOO-style (for example `[{op:"get_selection"}, {op:"set_text", ids:"$0.ids", text:...}]`), or a tiny DSL interpreted in Rust. That gives the composability of code actions without embedding Python.
- An architect/editor split fits the existing model mix: Sonnet (or DeepSeek) plans, and Haiku or the local model executes simple steps.
- Small local models struggle to write correct code or XML (see BFCL below). Code actions suit Sonnet and Haiku better than a tiny local model.

### Gaps
- I did not research Cline in this session (its plan/act mode split is documented by Cline but not fetched here). OpenHands internals (event stream, AgentSkills) are not verified beyond the abstract.

## 4. Writing system prompts and tool descriptions for general capability; evidence on description quality

### Takeaway
Anthropic calls tool descriptions "by far the most important factor in tool performance". Write each as a 3–4+ sentence explanation of what the tool does, when to use it and when not to, what each parameter means, and its limits. Keep the system prompt to principles at the "right altitude" with a few canonical examples, not if/else rules. Measured effects: tool-use examples took complex-parameter accuracy from 72% to 90%; a better search-tool description removed a biased behavior; SWE-bench state of the art came after description refinements.

### Cited Findings
- "Provide extremely detailed descriptions. This is by far the most important factor in tool performance."
  - Cover what the tool does, when it should be used (and when it shouldn't), what each parameter means, and caveats. "Aim for at least 3–4 sentences."
  - Consolidate related operations into fewer tools with an `action` parameter.
  - Namespace names with prefixes.
  - Return only high-signal, stable identifiers.
  - Use `input_examples` for complex inputs, at about 20–50 tokens each, or 100–200 for nested ones.
  — [Claude docs, Define tools](https://platform.claude.com/docs/en/agents-and-tools/tool-use/implement-tool-use)
- Writing tools for agents:
  - Build "a few thoughtful tools targeting specific high-impact workflows". Consolidate (`schedule_event` instead of list_users + list_events + create_event) and avoid overlapping tools.
  - Return natural-language names, not uuids. Resolving ids to names or a 0-indexed scheme "significantly improves" precision.
  - Offer `response_format: concise|detailed`. In their example detailed took 206 tokens and concise 72; for Slack, concise used about one-third of the tokens.
  - Claude Code caps tool responses at 25,000 tokens.
  - Error messages should give "specific, actionable" fixes with examples of correct input, not tracebacks.
  - Use unambiguous parameter names (`user_id`, not `user`).
  - "Even small refinements to tool descriptions can yield dramatic improvements". Claude appended "2025" to search queries until the description was fixed.
  - Evaluate with realistic multi-call tasks, held-out sets, and transcripts fed back to Claude to rewrite the tools. Claude-optimized tools sometimes beat expert-written ones.
  — [Anthropic, Writing effective tools for AI agents, 11 Sep 2025](https://www.anthropic.com/engineering/writing-tools-for-agents)
- Tool Use Examples improved complex-parameter accuracy from 72% to 90% in internal tests. Use 1–5 examples per tool. — [Anthropic, Advanced tool use, Nov 2025](https://www.anthropic.com/engineering/advanced-tool-use)
- System prompt guidance: avoid "hardcoding complex, brittle logic"; use diverse canonical examples, not edge-case lists; start minimal and add only to fix observed failures. — [Effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
- BFCL V4 format sensitivity (26 prompt variants, 200 entries):
  - JSON-described functions give the highest accuracy across nearly all models, then XML, then Python.
  - Small models score higher emitting JSON or Python than XML. Claude 3.7 Sonnet and tool-tuned models drop sharply with XML output.
  - Llama-3.1-8B drops significantly with extra tool-call tags.
  - Markdown vs plain text and rephrasing showed no consistent effect.
  - Common errors: wrong type names ("int" vs "integer"), bad JSON escaping, unclosed tags.
  — [Berkeley Gorilla, BFCL V4 prompt variation](https://gorilla.cs.berkeley.edu/blogs/17_bfcl_v4_prompt_variation.html)
- τ-bench: GPT-4o succeeds on under 50% of tool+policy tasks, and pass^8 is under 25% in retail. Reliability across repeated trials is the weak spot, which motivates rules being followed reliably. — [Yao et al., τ-bench, arXiv 2406.12045 (2024)](https://arxiv.org/abs/2406.12045)

### Inferences
- Rewrite the PixPin prompt as:
  1. A role and environment description: canvas, elements with ids and types, notes, documents.
  2. A working method: inspect, plan if multi-step, act, verify, report.
  3. Explicit permission to combine tools and use its own language skills (translate, summarize, rewrite) on text it reads, then write the result back with edit tools.
  4. 2–4 canonical worked examples showing composition, not a request→tool table.
- Move each routing rule into the matching tool's description as "Use when… Do not use for… Pair with…". The "say I have no tool" failure mostly comes from no tool advertising itself as a generic text editor of existing elements.
- Keep native JSON tool calling for every model. For the local model, avoid XML or custom tag formats (BFCL).
- Measure with pass^k over a small task set (for example "translate the figure", "make all red boxes blue", "align these"). Single runs hide unreliability.

### Gaps
- No public numeric before/after for Anthropic's Slack/Asana optimized tools; the post shows charts only. No study found that directly compares a routing-list prompt with a principles prompt on the same tools.

## 5. Handling many tools: dynamic selection, tool search, tool RAG, small models

### Takeaway
Accuracy falls as the tool list grows, especially for small models. The fix is deferred or retrieved tools: always load 3–5 core tools and fetch the rest by search. Measured gains are large: Opus 4 went from 49% to 74% with Tool Search, and RAG-MCP took Qwen-max from 13.6% to 43.1%. Around 40 tools is right at the threshold where this starts to pay.

### Cited Findings
- Anthropic Tool Search Tool:
  - 58 tools across 5 MCP servers cost about 55K tokens. Traditional loading reached about 77K before work started, against about 8.7K with search, roughly 85% fewer tokens.
  - MCP-eval accuracy: Opus 4 rose from 49% to 74%, and Opus 4.5 from 79.5% to 88.1%.
  - Use it when definitions exceed about 10K tokens or there are 10+ tools. "Keep your three to five most-used tools always loaded, defer the rest."
  — [Anthropic, Advanced tool use, 24 Nov 2025](https://www.anthropic.com/engineering/advanced-tool-use)
- The API supports `defer_loading` per tool, plus namespacing that is "especially important when using tool search". — [Claude docs, Define tools](https://platform.claude.com/docs/en/agents-and-tools/tool-use/implement-tool-use)
- RAG-MCP (retrieve relevant tool descriptions before the LLM call, base model qwen-max):
  - Selection accuracy was 43.13% with retrieval, against 13.62% with all descriptions in the prompt and 18.20% with keyword pre-filtering.
  - Prompt tokens averaged 1,084 against 2,134, about −49%; the abstract says "over 50%".
  - In the stress test, success stays above 90% while the correct tool sits below position 30, becomes intermittent between 31 and 70, and mostly fails beyond about 100.
  — [Gan & Sun, RAG-MCP, arXiv 2505.03275 (2025)](https://arxiv.org/html/2505.03275v1)
- Less is More (edge devices, fine-tuning-free): offering the small model only a recommended subset of tools per query cut execution time by up to 70% and power by up to 40%, and improved success. A secondary summary cites +35–40% tool accuracy; that figure is not verified against the paper. — [Paramanayakam et al., arXiv 2411.15399 (Nov 2024)](https://arxiv.org/html/2411.15399v1)
- A blog reports a 1.7B Qwen inventing tool names when given a 107-tool server, and doing better with smaller servers. This is a single anecdote from a vendor blog. — [Speakeasy, Why less is more for MCP](https://www.speakeasy.com/mcp/tool-design/less-is-more.md)

### Inferences
- With about 40 tools, Sonnet and Haiku will likely cope with all tools loaded, provided descriptions are good and caching keeps cost down. Deferred loading of rare tools (CAD, BIM, PDF) through Anthropic tool search is still worth trying.
- For the small local model, pre-select about 5–10 tools per turn: embed the user message plus selection type, retrieve by description, and always include core tools such as get_state/selection, update_element and add_element. Expect large gains given RAG-MCP and Less is More.
- Consolidating near-duplicate tools also shrinks the list (Section 6) and is the first step.

### Gaps
- No measurements were found specifically for Haiku 4.5 or DeepSeek on tool-count scaling. The RAG-MCP heat map gives no exact collapse threshold.

## 6. Generic "edit existing elements" tools and composability (small orthogonal tools that return ids/state)

### Takeaway
The evidence favors a small set of orthogonal, high-level tools:
- Read and find tools that return compact state with stable, human-meaningful ids.
- Generic mutate tools (update text or style or geometry on a list of ids, with an action enum).
- Creation tools that return the new ids.

Each result should include enough state for the next step and should fail with actionable guidance.

### Cited Findings
- Consolidate related operations into one tool with an `action` parameter. "Fewer, more capable tools reduce selection ambiguity." — [Claude docs, Define tools](https://platform.claude.com/docs/en/agents-and-tools/tool-use/implement-tool-use)
- Avoid overlapping tools. Return high-signal fields and resolve opaque ids to names or a 0-indexed scheme, which reduces hallucination. Paginate, filter and truncate with sensible defaults. Errors should steer toward correct use. — [Writing tools for agents](https://www.anthropic.com/engineering/writing-tools-for-agents)
- Just-in-time context: keep lightweight identifiers in context and load details through tools when needed. — [Effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
- Results that reference earlier outputs (ReWOO variables, LLMCompiler arguments that depend on earlier tasks) and code actions make composition cheap. — [LangChain, Planning agents](https://www.langchain.com/blog/planning-agents); [Advanced tool use](https://www.anthropic.com/engineering/advanced-tool-use)
- SWE-agent evidence: a bounded viewer (100 lines) beats both too little and the full content, and summarized search beats iterative search. — [SWE-agent](https://arxiv.org/html/2405.15793)
- Poka-yoke: design parameters so they are hard to misuse, as with the absolute-path change. — [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)

### Inferences
- A candidate PixPin core set:
  - `canvas_query(filter, fields, limit)` returns `[{id, type, text≤80 chars, bbox, style}]`, with a "selection" shortcut.
  - `canvas_update(ids[], patch{text?, style?, x?, y?, w?, h?, ...})`, possibly with `ops[]` of per-id patches. This covers translate, restyle, move and rename in one generic tool.
  - `canvas_add(elements[])` returns the new ids.
  - `canvas_delete(ids[])`.
  - `canvas_render()` returns an image for visual verification.
  - Specialist tools (OCR, trace image, chart) return element ids or text that feed the generic ones.
- Every mutating result should echo what changed ("updated 3 elements: ...") and the current values. Errors should name the offending id and suggest `canvas_query`.
- This lets the model solve unlisted requests by composition: OCR, then reason, then update; or query, then translate, then update.

### Gaps
- No published benchmark compares "one generic update tool" against "many specific edit tools" for canvas or Excalidraw-like editors. The recommendation rests on Anthropic's general guidance and SWE-agent's interface results, not on a canvas-domain measurement.
