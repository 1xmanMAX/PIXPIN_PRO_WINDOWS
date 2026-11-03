# Persistent memory, learned recipes and context management for a multi-model agent (PixPin Asistente)

## 1. Memory architectures (MemGPT/Letta, mem0, Zep/Graphiti, LangMem, Anthropic memory tool, OpenAI memory): designs and measured results

### Takeaway
Every serious design separates (a) a small always-in-context block of core facts/preferences, (b) a searchable store of past conversations and longer notes, and (c) an update step that adds, merges or deletes facts instead of only appending. On LOCOMO, extracted-fact memory (mem0) costs about 90% fewer tokens than full context and loses roughly 4 to 6 accuracy points. A plain-file store searched by a capable agent scored *higher* than mem0 (74.0% vs 68.5%), so the retrieval mechanism matters less than the agent knowing when to look.

### Cited Findings
**MemGPT (arXiv 2310.08560) / Letta**
- MemGPT's main context = read-only system instructions + a fixed-size read/write "working context" for key facts and preferences (editable only via function calls) + a FIFO message queue whose first entry is a recursive summary of evicted messages. External context = recall storage (every message, retrievable by function call) and archival storage (arbitrary-length text) — [MemGPT paper](https://arxiv.org/html/2310.08560)
- A memory-pressure warning at about 70% of the window lets the LLM save important information before eviction. At 100% the queue manager evicts about 50% of messages and updates the recursive summary. Evicted messages stay in recall storage — [MemGPT paper](https://arxiv.org/html/2310.08560)
- Deep memory retrieval accuracy: GPT-3.5 Turbo 38.7% to 66.9%, GPT-4 32.1% to 92.5%, GPT-4 Turbo 35.3% to 93.4% with MemGPT. On nested key-value retrieval, MemGPT+GPT-4 held up from 0 to 4 nesting levels, while plain GPT-4 hit 0% by level 3 — [MemGPT paper](https://arxiv.org/html/2310.08560)
- Letta memory blocks are always in context and have `label`, `description` (the docs call it the most important field, because the agent uses it to decide how to read and write the block), `value`, a `limit` in characters, and an optional `read_only` flag. One block can be attached to several agents via `block_ids`, so all of them see an update. Writes replace the whole value, and with concurrent writers the last write wins — [Letta docs: memory blocks](https://docs.letta.com/guides/agents/memory-blocks)
- Letta's "is a filesystem all you need" test: the LoCoMo history went into one file, and a GPT-4o-mini agent with grep/search_files/open/close tools reached 74.0%, against 68.5% for mem0's best (graph) variant. Letta's conclusion is that memory depends mostly on whether the agent knows when and how to retrieve, not on the mechanism. They also question how meaningful current memory benchmarks are — [Letta blog](https://www.letta.com/blog/benchmarking-ai-agent-memory)
- Third-party Letta scores (74% LoCoMo, 83.2% LongMemEval vs 49.0% for Mem0) come from a competitor page and an unnamed tester, so they are unverified — [MemoryLake](https://www.memorylake.ai/products/compare/vs-letta); [vantaige](https://vantaige.io/es/ai-tool/letta)

**mem0 (arXiv 2504.19413)**
- Extraction phase: for each new message pair, the LLM gets the conversation summary plus the last m=10 messages and extracts candidate facts. An async module refreshes the summary. Update phase: for each fact, the top s=10 similar memories are retrieved by embedding, and the LLM picks ADD, UPDATE, DELETE (for contradicted memories) or NOOP through a tool call. GPT-4o-mini runs both phases — [mem0 paper](https://arxiv.org/html/2504.19413v1)
- LOCOMO overall LLM-as-judge (J) scores: Full-context 72.90%, Mem0g (graph) 68.44%, Mem0 66.88%, Zep 65.99%, best RAG (k=2, 256-token chunks) 60.97%, LangMem 58.10%, OpenAI memory 52.90%, A-Mem 48.38% — [mem0 paper](https://arxiv.org/html/2504.19413v1)
- p95 total latency: OpenAI 0.889 s, Mem0 1.440 s, RAG 1.907 s, Mem0g 2.590 s, Zep 2.926 s, A-Mem 4.374 s, full-context 17.117 s, LangMem 60.40 s. Memory footprint per conversation: Mem0 about 7k tokens, Mem0g about 14k, Zep over 600k, raw conversation about 26k — [mem0 paper](https://arxiv.org/html/2504.19413v1)
- Abstract claims: 26% relative J improvement over OpenAI, 91% lower p95 latency and more than 90% token savings vs full context — [mem0 abstract](https://arxiv.org/abs/2504.19413)
- Note: these are mem0's own numbers. Letta says mem0 did not explain how its numbers were computed — [Letta blog](https://www.letta.com/blog/benchmarking-ai-agent-memory)

**Zep / Graphiti (arXiv 2501.13956)**
- Graphiti is "a temporally-aware knowledge graph engine" that combines conversation data with business data and keeps historical relationships. Zep reports 94.8% on DMR vs 93.4% for MemGPT, and on LongMemEval up to 18.5% higher accuracy with 90% lower latency than baselines — [Zep paper](https://arxiv.org/abs/2501.13956)
- In mem0's evaluation, Zep's memory footprint was over 600k tokens per conversation — [mem0 paper](https://arxiv.org/html/2504.19413v1)

**LangMem**
- Three memory types. Semantic memory is either *collections* (many records, searched and consolidated or invalidated) or *profiles* (a single current-state document that is updated in place). Episodic memory stores successful interactions (situation, reasoning, action, result). Procedural memory is the system prompt, refined by prompt optimization from feedback — [LangMem conceptual guide](https://langchain-ai.github.io/langmem/concepts/conceptual_guide/)
- Memories can form on the "hot path" (during the chat: immediate but adds latency) or in the "background" (reflection after the conversation goes idle: no added latency, deeper analysis, delayed). Over-extraction hurts precision and under-extraction hurts recall. Retrieval should weigh similarity, importance and "strength" (how recently and how often a memory was used). Namespaces are hierarchical (org/user/app) — [LangMem conceptual guide](https://langchain-ai.github.io/langmem/concepts/conceptual_guide/)

**Anthropic memory tool and context editing**
- The memory tool (`memory_20250818`) is client-side. Claude issues view/create/str_replace/insert/delete/rename on paths under `/memories`, and the application runs them on storage it controls (files, a DB, encrypted files). It works on all Claude 4+ models — [Claude docs: memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- When the tool is present, the API injects: "ALWAYS VIEW YOUR MEMORY DIRECTORY BEFORE DOING ANYTHING ELSE… ASSUME INTERRUPTION: Your context window might be reset at any moment". `view` truncates text over 16,000 characters and supports `view_range` — [Claude docs: memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- The security guidance assigns the app several jobs: strip sensitive data, cap file sizes, expire long-unused files, and block path traversal (`../`, `..\\`, URL-encoded `%2e%2e%2f`) by canonicalizing paths and checking they stay under the root — [Claude docs: memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- The recommended multi-session pattern: an initializer session writes a progress log and a feature checklist, each later session reads them first, and the log is updated before the session ends — [Claude docs: memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- Anthropic reports that memory tool + context editing improved an agentic-search eval by 39% over baseline, and context editing alone by 29%. In a 100-turn web-search eval, context editing cut token use by 84% and let agents finish workflows that otherwise ran out of context — [Anthropic: context management](https://www.anthropic.com/news/context-management)

**OpenAI (ChatGPT) memory**
- Two mechanisms: "saved memories" (explicit details like name, preferences and goals, saved on request or automatically) and "reference chat history" (insights from past chats that change over time). Turning chat history off schedules its derived info for deletion within 30 days. Temporary Chat neither reads nor updates memory — [OpenAI blog](https://openai.com/blog/memory-and-new-controls-for-chatgpt); [OpenAI help 11146739](https://help.openai.com/en/articles/11146739-how-does-reference-saved-memories-work); [OpenAI Memory FAQ](https://help.openai.com/en/articles/8590148-memory-faq)
- OpenAI memory scored lowest but one on LOCOMO in mem0's test (52.90% J), with the lowest p95 latency (0.889 s) — [mem0 paper](https://arxiv.org/html/2504.19413v1)

### Inferences
- For PixPin, the MemGPT 70% warning lines up with the existing 70% reset. The missing piece is a "save to memory before reset" step: at 70%, the model (or a cheap background pass with Haiku) writes durable facts and recipes to long-term memory, and then the summary is made.
- The Letta filesystem result plus the Anthropic memory tool make a strong case that a vector DB is not a prerequisite. Text files with search tools reach or beat mem0 when the model is capable. The small local model is the exception: it is weaker at deciding when to search, so it benefits from the app pre-injecting the core block plus top-k retrieved items, the way mem0 does.
- mem0's ADD/UPDATE/DELETE/NOOP step is the cheapest proven way to stop duplicate and contradictory facts piling up. It runs well on a small cheap model (GPT-4o-mini in the paper), so Haiku or DeepSeek should be able to do it in the background.

### Gaps
- I found no head-to-head benchmark of the same memory store shared across *different* model families (Claude vs DeepSeek vs a small local model). Model-agnostic sharing is a design argument, not a measured result.
- I did not confirm the Zep paper's bi-temporal edge details from the full text.
- Letta and mem0 dispute each other's numbers, and neither benchmark is independent.

## 2. Model-agnostic memory formats (markdown/JSON files via tools) vs vector DB for a local-first desktop app with sync

### Takeaway
Plain markdown with a small always-loaded index plus on-demand topic files is what Anthropic ships (Claude Code auto memory, the memory tool). It is model-agnostic because any model can read text through generic file tools, it is human-editable, and it syncs like any file. Add embeddings only as an optional index, never as the source of truth.

### Cited Findings
- Claude Code auto memory is a per-project directory with a `MEMORY.md` index (one line per memory) plus topic files. Only the first 200 lines or 25KB of `MEMORY.md` load at session start. Topic files are read on demand with ordinary file tools. Everything is plain markdown the user can edit or delete, and `/memory` browses it — [Claude Code docs: memory](https://code.claude.com/docs/en/memory)
- Claude Code advises keeping a CLAUDE.md under 200 lines, because longer files "consume more context and reduce adherence" — [Claude Code docs: memory](https://code.claude.com/docs/en/memory)
- Memory files are excluded from Claude Code's transcript retention sweep and persist until edited or deleted — [Claude Code docs: memory](https://code.claude.com/docs/en/memory)
- The memory tool's `/memories` prefix is a virtual path. The handler can map it to a per-user directory or to database keys — [Claude docs: memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- Just-in-time retrieval: agents keep lightweight references (paths, queries) and load content with tools. Claude Code is a hybrid that loads CLAUDE.md up front and uses glob/grep for the rest — [Anthropic: effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
- A filesystem agent with grep plus semantic search over a file reached 74.0% on LoCoMo — [Letta blog](https://www.letta.com/blog/benchmarking-ai-agent-memory)
- Letta's shared blocks use last-write-wins on concurrent writes — [Letta docs](https://docs.letta.com/guides/agents/memory-blocks)

### Inferences
- Proposed PixPin layout (synced like the Asistente chat):
  - `memoria/PERFIL.md`: always loaded, 2 to 4 KB cap. User preferences and stable facts.
  - `memoria/INDICE.md`: always loaded, one line per entry, about 200-line cap.
  - `memoria/hechos/*.md` and `recetas/<nombre>/RECETA.md`: loaded on demand.
  - Each entry carries YAML front matter: id, created, updated, last_used, uses, source (model + conversation id), confidence, and a `pinned` flag set by the user.
- Expose the store to every model through one tool set implemented in Rust: view, create, str_replace, delete, search. For Claude, map the Anthropic `memory_20250818` command set onto the same handler, so Claude gets its trained behavior. DeepSeek and the local model get the same commands as ordinary function tools.
- Search: SQLite FTS5 (BM25) over the markdown files needs no model and runs locally. Embeddings could be an optional rebuildable cache. I did not research sqlite-vec or Rust embedding crates specifically.
- Sync: plain files merge per entry. One file per memory avoids the last-write-wins problem Letta warns about with one big shared block. A conflict leaves both versions for the consolidation pass to merge.

### Gaps
- No measured comparison of FTS/BM25 vs embeddings for personal-assistant memory at small scale (hundreds of entries).
- I found no evidence on how well small local models (1 to 4B parameters) drive file-based memory tools.

## 3. Procedural memory and skill libraries (Voyager, Agent Skills SKILL.md, AWM, ExpeL): storage, retrieval, validation, evidence of gains

### Takeaway
Reusable procedures give large gains when they are verified before being saved (Voyager, ExpeL) or curated (SkillsBench). In SkillsBench, self-written skills that were never checked gave no benefit on average. Store recipes as small SKILL.md-style files. Show only name + description up front, load the body on demand, and promote a recipe only after it has worked and the user or a check confirms it.

### Cited Findings
- Voyager keeps an ever-growing library of executable code skills, indexed by embeddings of their descriptions. Skills are refined using environment feedback, execution errors and self-verification before they are kept. Against prior SOTA: 3.3x more unique items, 2.3x longer distances, tech-tree milestones up to 15.3x faster. The library transfers to a new world — [Voyager](https://arxiv.org/abs/2305.16291)
- Agent Workflow Memory (AWM) induces reusable "workflows" from past experiences, offline from training examples or online from test queries, and supplies them selectively. It reports +24.6% relative success on Mind2Web and +51.1% on WebArena with fewer steps, and online AWM's gain grows from 8.9 to 14.0 points as the train-test gap widens — [AWM](https://arxiv.org/abs/2409.07429)
- ExpeL collects trajectories, distills natural-language insights managed with ADD/EDIT/UPVOTE/DOWNVOTE, and retrieves successful trajectories as few-shot examples, with no weight updates — [ExpeL abstract](https://arxiv.org/abs/2308.10144)
- ExpeL success rates: HotpotQA ReAct 28.0% to ExpeL 39.0%, ALFWorld 40.0% to 59.0%, and ALFWorld with Reflexion retries 54.4% to 64.2%. Transfer to FEVER: 70% vs ReAct 63%. In the ablation, insights mattered most on HotpotQA (36% insights-only vs 31% retrieval-only) and retrieved examples mattered more on ALFWorld (55% vs 50%) — [ExpeL paper](https://arxiv.org/html/2308.10144v3)
- Agent Skills: a directory with `SKILL.md` (YAML front matter with required `name` and `description`, then instructions) plus optional scripts and reference files. Progressive disclosure:
  - Level 1: name and description sit in the system prompt.
  - Level 2: the full SKILL.md is read when relevant.
  - Level 3: linked files are read only when needed.
  - Anthropic publishes it as an open standard for cross-platform portability. Bundled scripts make steps deterministic. The `name` and `description` decide when the skill triggers. Anthropic recommends having the agent fold successful approaches and mistakes back into the skill — [Anthropic engineering: Agent Skills](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills)
- SkillsBench (arXiv 2602.12670; 86 tasks, 11 domains, 7,308 trajectories):
  - Curated skills raise pass rate by 16.2 pp on average, ranging from +4.5 pp (software engineering) to +51.9 pp (healthcare), and 16 of 84 tasks got worse.
  - Self-generated skills give no benefit on average.
  - Focused skills with 2 to 3 modules beat comprehensive documentation.
  - Smaller models with skills can match larger models without them.
  - [SkillsBench](https://arxiv.org/html/2602.12670v1)
- LangMem's episodic memory stores successful interactions (situation, reasoning, action, result) as learning examples — [LangMem](https://langchain-ai.github.io/langmem/concepts/conceptual_guide/)

### Inferences
- Recipe lifecycle for PixPin:
  1. After a turn where a multi-step tool sequence succeeded (the worksheet shows it), a background pass with Haiku drafts a candidate recipe: name, one-line "when to use", steps as tool calls with parameters, and pitfalls.
  2. The candidate is saved as `borrador` and is not offered automatically.
  3. It becomes `activa` only after it is reused successfully (deterministic check: the tools returned OK and the user did not undo) or after the user approves it in the memory panel.
  4. Downvote or retire it on failure (ExpeL-style counters).
  - SkillsBench's null result for unverified self-generated skills is the main reason for the draft stage.
- Keep recipes small and focused, since SkillsBench found 2 to 3 modules beat long docs. Where possible, store the steps as a replayable tool-call script, as Voyager does with code, rather than prose. This makes a recipe executable by any model, including the small local one. SkillsBench's "small model + skills ≈ large model" finding suggests recipes help the local model most.
- Retrieval: put the names and descriptions of active recipes in the always-loaded index (Level 1). Load the body only when chosen, or when FTS ranks it top for the user's request.

### Gaps
- No evidence found on the success rate of skills written by one model family and executed by another.
- AWM's absolute success rates were not in the abstract.

## 4. Context management: compaction strategies, what to keep, "new conversation" semantics, context rot

### Takeaway
Use layered compaction: first clear old tool results (cheapest), then summarize at a threshold into a structured summary, and keep durable facts in external memory so they survive. A new conversation should start with only system prompt + core memory block + recipe index, with the old chat reachable by search ("recall"), not preloaded.

### Cited Findings
- Context rot: Chroma tested 18 models (GPT-4.1, Claude 4, Gemini 2.5, Qwen3). Even on simple tasks, reliability drops as input length grows, and models do not use their context uniformly — [Chroma: Context Rot](https://trychroma.com/research/context-rot)
- Anthropic describes an "attention budget". Recall degrades as tokens grow, gradually rather than all at once, because of n² token relationships and less training on long sequences — [Anthropic: effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
- Compaction guidance:
  - Keep architectural decisions, unresolved bugs and key details. Drop redundant tool outputs.
  - Tune the summary prompt for recall first, then for precision.
  - Clearing old tool results is the "lightest-touch" compaction.
  - Sub-agents return condensed summaries of 1,000 to 2,000 tokens.
  - [Anthropic: effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
- Claude API context editing, `clear_tool_uses_20250919`: `trigger` (default 100k input tokens), `keep` (default last 3 tool uses), `clear_at_least` (so a clear is worth breaking the prompt cache), `exclude_tools`, `clear_tool_inputs`. `clear_thinking_20251015` keeps the last N thinking turns. Clearing invalidates the cached prefix — [Claude docs: context editing](https://platform.claude.com/docs/en/build-with-claude/context-editing)
- The default compaction summary covers task overview, current state, important discoveries, next steps and context to preserve. It can run on a cheaper model. Server-side compaction (`compact_20260112`) is now recommended. When context nears the clearing threshold, Claude is warned and can save to memory first — [Claude docs: context editing](https://platform.claude.com/docs/en/build-with-claude/context-editing)
- For long agents Anthropic suggests using both: compaction keeps active context small, and memory "preserves the information that must survive summarization" — [Claude docs: memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- OpenHands `LLMSummarizingCondenser` (`max_size`, `keep_first`): always keeps the first N events (system prompt, initial user message) and the recent events, and replaces the middle with an LLM summary of goals, specs and critical files — [OpenHands docs](https://docs.openhands.dev/sdk/guides/context-condenser)
- OpenHands results: per-turn cost under half of baseline (up to 2x cheaper). Cost scales linearly instead of quadratically. SWE-bench Verified subset 54% with the condenser vs 53% without, at the price of extra turns spent condensing — [OpenHands blog](https://www.openhands.dev/blog/openhands-context-condensensation-for-more-efficient-ai-agents)
- MemGPT keeps the recursive summary as the first queue item and evicted messages in searchable recall storage, so nothing is lost, only moved out of context — [MemGPT paper](https://arxiv.org/html/2310.08560)
- ChatGPT's Temporary Chat neither reads nor writes memory, so it is a separate "clean and private" mode — [OpenAI help](https://help.openai.com/en/articles/8590148-memory-faq)

### Inferences
- PixPin's 70%-of-64k reset should become three steps:
  1. From about 50%, drop old tool results and worksheet bodies, keeping the last 3, as cheap clearing that also preserves the cache prefix longer.
  2. At 70%, run a "save durable facts" pass to memory, then a structured summary (OpenHands-style keep_first + recent + summarized middle).
  3. Store the raw turns in the synced chat log as recall memory, searchable through a `buscar_conversaciones` tool.
- "Nueva conversación" semantics:
  - Start with system prompt + PERFIL.md + INDICE.md (facts and recipe names) and nothing from the previous chat.
  - Optionally add a one-line pointer: "previous conversation: <title>, searchable".
  - Run the memory-extraction pass on the closed conversation in the background, LangMem's "subconscious" mode, so the button has no latency.
  - Offer a "temporal" mode that skips memory reads and writes, like ChatGPT Temporary Chat.
- Because the window and tokenizers differ between models, set the thresholds per model in tokens measured by that model's counter, not as one fixed number.

### Gaps
- No published numbers found for Claude Code's own auto-compact threshold or summary quality.
- No head-to-head study of summary-based compaction vs clear-and-recall for chat assistants, as opposed to coding agents.

## 5. Risks: memory poisoning, stale memories, privacy; user visibility and editing

### Takeaway
Persistent memory is an attack surface. Query-only injection reached a 98.2% injection rate in research settings. Treat memory writes as untrusted, record each memory's source, let only user-originated content become a "fact", review recipes, and give the user a panel to see, edit, pin and delete every entry, plus expiry for unused items.

### Cited Findings
- MINJA (arXiv 2503.03704) injects malicious memory records through ordinary queries alone. In idealized conditions, 98.2% of injections succeeded and 76.8% of attacks went on to trigger malicious reasoning. Unlike AgentPoison, it needs no direct database access — [MINJA](https://arxiv.org/html/2503.03704v2); [The Register](https://www.theregister.com/2025/03/11/minja_attack_poisons_ai_model_memory/)
- A follow-up on EHR agents notes that MINJA's rates were measured in idealized conditions and that robustness in realistic deployments is understudied — [arXiv 2601.05504](https://arxiv.org/pdf/2601.05504v1)
- AgentPoison is reported at 80% or higher attack success with a poison rate under 0.1% and under 1% impact on benign queries. That figure is second-hand — [Vectorize summary](https://vectorize.io/articles/ai-memory-poisoning)
- Skills can introduce vulnerabilities or exfiltrate data. Install only from trusted sources and audit bundled scripts and network instructions — [Anthropic: Agent Skills](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills)
- The memory tool docs assign the app four jobs: strip sensitive data before writing, cap sizes, expire unused files, and guard against path traversal — [Claude docs: memory tool](https://platform.claude.com/docs/en/agents-and-tools/tool-use/memory-tool)
- Visibility and editing:
  - Claude Code memory is plain markdown, browsable and editable with `/memory`, with a toggle for auto memory — [Claude Code docs](https://code.claude.com/docs/en/memory)
  - ChatGPT has Settings > Personalization controls to manage or delete saved memories and to switch chat-history referencing off — [OpenAI help](https://help.openai.com/en/articles/11146739-how-does-reference-saved-memories-work)
- Stale or contradictory facts are handled in mem0 by an explicit DELETE when new info contradicts old, and in LangMem by invalidating or consolidating, with retrieval weighted by recency and frequency ("strength") — [mem0 paper](https://arxiv.org/html/2504.19413v1); [LangMem](https://langchain-ai.github.io/langmem/concepts/conceptual_guide/)

### Inferences
- PixPin rules:
  1. Only text the user typed or explicitly confirmed can create or update PERFIL facts. Content from tool results, web pages, OCR or pasted files can at most create a `borrador` note tagged with its source. This blocks the MINJA/indirect-injection path.
  2. Recipes containing shell, file-write or network steps need user approval before they become `activa`.
  3. Each entry records `fuente` (model + conversation + date), `ultimo_uso` and `usos`. A background job flags entries unused for N days, and proposes merges or deletions with mem0-style operations, applied only after the user accepts.
  4. A "Memoria" panel in the Asistente lists facts and recipes, with edit, pin, delete, "no recordar esto" and a global off switch. Show a "Recordado: …" chip when a turn writes memory, as Claude Code does with "Saved N memories".
  5. Use a secret-pattern filter (API keys, passwords) in the Rust write handler before anything syncs. Keep memory per user and do not send it to a cloud model if the user marks an entry "solo local".
- Since all models share one store, a weak or compromised model could corrupt memory for the strong ones. Route consolidation writes through one designated model (for example Haiku in the background) and keep a version history (one file per entry, with old versions kept) so the user can roll back.

### Gaps
- No measured data on how often benign agents write wrong or stale memories in practice. Vendors do not publish error rates.
- No primary-source AgentPoison numbers retrieved; the figure above is second-hand.
