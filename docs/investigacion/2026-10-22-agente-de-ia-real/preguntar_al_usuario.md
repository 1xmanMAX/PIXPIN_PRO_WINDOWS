# Asking clarifying questions instead of guessing: research and agent-loop design for PixPin's assistant

## 1. What does research (2023–2026) say about when asking helps, and about the trade-off between asking and acting?

### Takeaway
Across code generation, QA, preference-sensitive tasks and coding agents, a single targeted clarification on genuinely ambiguous inputs gives large gains, often 7 to 20+ points. The dominant failure of current LLMs is *under*-asking: they pick one interpretation and answer. Over-asking does exist, though, and it shows up mainly in models trained hard to ask. The best results come from a separate "is this ambiguous?" detection step, done by sampling several interpretations or outputs and checking whether they disagree, followed by one focused question.

### Cited Findings
**Detection plus a targeted question (prompting, no training)**
- ClarifyGPT (Mu et al., FSE 2024) samples several code solutions and runs a code-consistency check. Only when the solutions disagree is the requirement treated as ambiguous and a targeted question generated. GPT-4 Pass@1 on MBPP-sanitized rose from 70.96% to 80.80%. Averaged over 5 benchmarks with a simulated user, GPT-4 went from 62.43% to 69.60% and ChatGPT from 54.32% to 62.37%. — [arXiv 2310.10996](https://arxiv.org/abs/2310.10996v1); [FSE 2024](https://2024.esec-fse.org/details/fse-2024-research-papers/89/ClarifyGPT-A-Framework-for-Enhancing-LLM-based-Code-Generation-via-Requirements-Clar)
- CLAM (Kuhn, Gal, Farquhar; ICML 2023 workshop) works in three steps: detect whether the question is ambiguous, generate a clarifying question, then answer after the user replies. It uses only few-shot prompting. The authors found that models "rarely ask users to clarify ambiguous questions and instead provide incorrect answers". CLAM significantly raised accuracy on ambiguous questions without hurting unambiguous ones. — [arXiv 2212.07769](https://arxiv.org/abs/2212.07769v2)
- "Clarify When Necessary" (Zhang & Choi, Findings NAACL 2025) splits the problem into *whether* to ask, *what* to ask, and how to answer with the clarification. Its Intent-Sim method estimates the entropy over plausible user intents by simulating several user-assistant exchanges, and it beat other uncertainty estimators at finding the cases that benefit from asking. When asking was limited to 10% of examples, it roughly doubled the gain over choosing which examples to ask about at random. — [arXiv 2311.09469](https://arxiv.org/pdf/2311.09469); [ACL Anthology](https://preview.aclanthology.org/moar-dois/2025.findings-naacl.306)

**Training models to ask**
- STaR-GATE (Andukuri et al., Stanford, COLM 2024) builds 25,500 persona-task prompts. A "Questioner" model is fine-tuned on the questions that most raise the probability of a good answer, as judged by an oracle that knows the user's hidden preferences. After 2 iterations, its responses were preferred over the initial model's on 72% of tasks. — [arXiv 2403.19154](https://arxiv.org/abs/2403.19154v3)
- Zhang, Knox & Choi (ICLR 2025) explain under-asking this way: standard RLHF preference labels judge a response only on its preceding context, so "complete but presumptuous" answers beat clarifying questions. Labelling responses by simulating the *next* turns instead gave +5% F1 across the different interpretations, and +3% accuracy at deciding whether to ask or answer directly. — [arXiv 2410.13788](https://arxiv.org/abs/2410.13788v2)

**Benchmarks on under-asking vs over-asking**
- AskBench (Feb 2026) has two settings. AskMind removes key details from the question. AskOverconfidence plants confident false premises. The budget is 3 turns. Strong models under-ask: Gemini-2.5-Pro and GPT-4.1 resolved only about 12% of the AskMind checkpoints (coverage 0.124 and 0.118). On AskOverconfidence-Hard, Gemini answered on the first turn in 387 of 400 cases. RLVR training of Qwen2.5-7B raised AskMind accuracy from 0.332 to 0.615. **The over-asking cost:** on IN3, the trained models asked on 94 to 100% of vague tasks, but answered *clearly specified* tasks directly only 62.5% and 50% of the time, against 100% for the base model. Their redundant-question rate also climbed to 0.463 in one variant. — [arXiv 2602.11199](https://arxiv.org/html/2602.11199v1)
- "Ask or Assume?" (Edwards & Schuster, 2026) takes 500 SWE-bench Verified issues rewritten to be underspecified, with GPT-5.1 simulating the user. A separate Intent Agent, which only monitors for missing information and forces the main agent to ask, reached 69.4% resolved with Claude Sonnet 4.5. That is close to the fully specified ceiling of 70.8%, and the difference was not significant (p=0.458). A single agent with a recurring "check for ambiguity" reminder reached 61.2%. Claude asked in 68.8% of tasks and Kimi in 87%. On the tasks where Claude chose *not* to ask, results matched the no-clarification baseline (76.9% vs 77.6%), which suggests it abstained on easy tasks. On the tasks where it asked, it resolved 66.0% versus 44.5% without asking (correlational). **Cost:** the two-agent scaffold cost $3.50 per task against $2.03 for the single agent. — [alphaXiv 2603.26233](https://www.alphaxiv.org/abs/2603.26233)
- ClarifyMT-Bench (Dec 2025) evaluated 10 LLMs and found a consistent under-clarification bias: models answer prematurely, and performance degrades as dialogues get deeper. — [papers.cool 2512.21120](https://papers.cool/arxiv/2512.21120) (search snippet only, paper not opened)
- HumanAgencyBench reported a low mean score of 24.2% for asking clarifying questions, with older models under 10% and newer ones around 62 to 81%. — [arXiv 2509.08494](https://arxiv.org/pdf/2509.08494) (search snippet only)

### Inferences
- The gains concentrate on inputs that really are ambiguous. If every request triggers a question, the cost (latency, annoyance, wrong answers on clear tasks, as AskBench's IN3 numbers show) can cancel out the benefit. A **gate** is therefore more important than how well the questions are worded.
- The most robust gate that needs no training is **disagreement between samples**. ClarifyGPT compares generated code, and Intent-Sim compares simulated intents. For PixPin, this could mean asking a cheap model (Haiku, or even the local model) to list the 2 or 3 most plausible interpretations, or quick drafts of the canvas or note result. If the interpretations lead to materially different outputs, ask. If they agree, act.
- A *separate* monitor with a narrow job ("is something missing that changes the result?") clearly beat a reminder inside the main agent (+8 points on SWE-bench). It costs about 1.7x more on frontier models. With Haiku as the monitor, the extra cost would be small. This is an inference: the cited result used the same model for both roles.
- RLHF-tuned models lean toward guessing by default (Zhang et al.). The system prompt and the tool must therefore push explicitly toward asking when the gate fires.

### Gaps
- I found no study measuring how *annoyed real users get* by unnecessary questions, and no data on the time cost per extra turn. AskBench uses only proxies, such as the redundant-question rate.
- Nearly all of this evidence uses LLM-simulated users, not humans.
- The AmbigQA statistic on how many questions are ambiguous could not be verified from what I read.
- No paper covered canvas, drawing or notes tasks specifically.

## 2. How do production agents ask? (Claude Code AskUserQuestion, Cline, OpenAI deep research)

### Takeaway
Production agents turn "asking" into a **tool call that pauses the loop**. The questions are structured and multiple-choice, there is always a free-text escape, and the answer comes back as the tool result. Deep-research systems put a dedicated triage/clarification step *before* the expensive work starts.

### Cited Findings
- **Claude Agent SDK / Claude Code `AskUserQuestion`:**
  - Claude calls the tool "when it needs more direction on a task with multiple valid approaches", and execution pauses until the host returns answers.
  - Schema: `questions[]` holds 1 to 4 questions. Each has `question` (full text), `header` (at most 12 characters, shown as a chip), `options` (2 to 4, each with a `label` and a `description`), and `multiSelect`.
  - Optional `preview` (markdown or HTML) can show a visual mockup per option. Claude includes it only where a visual comparison helps.
  - The host adds an "Other" free-text choice and returns the typed text, not the word "Other".
  - Answers are keyed by question text. An optional `response` field holds a free-form reply when the user dismisses the card.
  - Questions are "especially common in plan mode", where Claude explores and then asks before proposing a plan.
  - The tool is not available to subagents. — [Claude Agent SDK docs: Handle approvals and user input](https://code.claude.com/docs/en/agent-sdk/user-input)
- Community guides recommend putting the recommended option first, with the label suffixed "(Recommended)". — [DEV Community deep dive](https://dev.to/_94be737e156beb4d74df2/claude-code-tools-deep-dive-1-askuserquestion-4ck1); [Dometrain](https://ai-workshop.dometrain.com/docs/getting-work-done/planning/ask-user-question) (secondary sources)
- **OpenAI deep research:**
  - OpenAI's API demo of a deep-research agent uses 4 agents, Triage → Clarification → Instruction → Research. A query is checked and clarified only if needed, then turned into a structured research brief. — [The Decoder](https://the-decoder.com/openai-is-teaching-developers-how-to-build-deep-research-agents/)
  - The ChatGPT help center says that in Chat mode the model proposes a research plan you can review and edit, while in other modes it asks clarifying questions after the first prompt. — [OpenAI Deep research FAQ](https://help.openai.com/en/articles/10500283-deepresearch-faq)
- **Cline:**
  - Cline's CLI output marks interactive messages with an ask subtype `followup`. — [Cline CLI reference](https://docs.cline.bot/cline-cli/cli-reference)
  - The newer ClineCore SDK lists an `ask_question` tool ("Ask the user for input") under human-in-the-loop controls. — [Cline tools guide](https://docs.cline.bot/exploring-clines-tools/cline-tools-guide)

### Inferences
- These systems share one pattern. The model calls `ask_user`, the loop suspends, the UI renders a card with options plus "Other", and the answer becomes the tool result. That fits PixPin's existing tool-use loop for both cloud models. For DeepSeek and the local model, the same JSON schema works through their function-calling support, or through a parsed fallback.
- The deep-research pattern, which clarifies *before* the expensive phase, maps onto PixPin's long jobs such as generating a whole canvas, a document or a chart: gate once at the start, then run without interruptions. This matches the user's preference: "Probar y avisar al final" ("test, then report only at the end"; his own memory note).
- Option previews matter for a canvas app. Two or three thumbnail variants of a layout or figure are a good fit for the `preview` idea.

### Gaps
- I could not retrieve the exact Cline `ask_followup_question` schema. The prompt file was not at the URL I tried (404). Its parameters are believed to be `question` plus an optional `options` array, but this is unverified.
- I found no public information on how Devin handles clarification.
- I found no official numbers on how often Claude Code or deep research ask questions in practice.

## 3. What are the rules and rationale of the superpowers "brainstorming" skill?

### Takeaway
The skill first triages: if everything that matters is in the request, just do it and say what was done. Otherwise it holds a conversation with **one open, concrete question per message**. It plays back its understanding in chunks of roughly 200 to 300 words, marks guesses as guesses, states any defaults out loud, and builds nothing until the user approves.

### Cited Findings (all from the local copy of superpowers 7.0.0, `skills/brainstorming/SKILL.md`, github.com/obra/superpowers)
- The rationale: the agent builds "what you believe your human partner wants, and that belief is usually thinner than it feels". Understanding the *why* lets it make "the hundred decisions they never mention" the way the user would.
- **Triage first.** "Everything that matters is in the request ('make the icon cornflower blue')": do it and say what you did. Only when "something non-trivial is missing" does the conversation start.
- **First message.** It has exactly two parts: one line offering to skip the questions, then one open question about a real moment or a concrete picture. If the user opts out, the agent states any significant how-it-gets-made choice in one line and proceeds.
- **One question per message.** Each message ends with one single-sentence question "with no list of example answers hanging off it". Questions use the user's own vocabulary and no process jargon.
- **Open questions before options.** "Options steer. Ask them to describe it first." A short menu is for when the user is stuck. On "just guess", propose something concrete.
- **A correctable guess often beats a question.** For example: "I'm guessing this is because X keeps biting you?"
- **Good questions anchor in specifics.** Examples: "Walk me through the last time…", "If it could only do one of these well, which?", "What would make you wince if I got it wrong?"
- **Defaults are said out loud.** "A default they never heard is one they couldn't reject." Any choice the agent makes is recorded as its own call, never as something the user agreed to.
- **Playback.** Each part is played back in about 200 to 300 words, alternating with questions. Guesses are marked as such, and each chunk ends with "what's wrong or missing?"
- **Size the work.** The agent sizes the work as a quick task (do it), a small change (describe it in chat), or a project (write a design doc). It says the size, and the user can override it.
- **Builder check.** A reviewer reads the design. Its open questions are sorted into already answered, minor (the agent settles them and marks them as its call), and "theirs" (only the user can answer, and the answer changes what gets built). The "theirs" pile is sent in a single message, most important first.
- **Show, don't tell.** A visual companion with mockups is offered just in time for anything the user will look at.
- **Hard gate.** Nothing is built before the full description is approved, unless the user opted out or asked for a throwaway spike.

### Inferences
- The skill's "minor vs theirs" split is the same policy as the research's "ask only when the answer changes the outcome". Minor gaps get a stated assumption, while "theirs" gaps get a question.
- For a chat assistant inside an app, the full brainstorming flow (playback chunks, design docs, the hard gate) is too heavy for most requests. Three parts carry over directly: the triage, one question at a time, and stated defaults. The deeper flow fits only "project-sized" requests, such as building a whole presentation or canvas.
- There is a tension. Brainstorming prefers *open* questions, while AskUserQuestion uses *multiple choice*. A reconciliation (inference): use options when the space of interpretations is small and enumerable (which canvas, which format, which of 3 detected readings). Use one open question when the user's goal itself is unclear.

### Gaps
- The skill gives no measured evidence for its rules. They are a practitioner's design, not an evaluated method.

## 4. Policies and agent-loop design: when to ask, when to assume and proceed, and thresholds

### Takeaway
Ask only when (a) plausible interpretations lead to materially different results, *and* (b) the context, project files or a quick look cannot settle it, *and* (c) a wrong guess is expensive: a long or costly generation, something destructive or irreversible, or work the user must redo. In every other case, act on the most likely reading and state the assumption in one line, so the user can correct it cheaply.

### Cited Findings
- Selective asking beats both never asking and always asking. Limiting clarification to the top 10% of cases by intent entropy roughly doubled the gain compared with picking at random. — [Zhang & Choi 2025](https://arxiv.org/pdf/2311.09469)
- Asking too readily costs accuracy on clear tasks. Ask-trained models answered clear IN3 tasks directly only 50 to 62.5% of the time, against 100% for the base model. — [AskBench](https://arxiv.org/html/2602.11199v1)
- Asking is pointless if the answer is not used. On AskMind, FATA-style prompting raised coverage to 0.503, but its accuracy stayed below the trained model's. — [AskBench](https://arxiv.org/html/2602.11199v1)
- A dedicated detector, separate from the executor, works better than a self-reminder: 69.4% vs 61.2% resolved. — [Ask or Assume?](https://www.alphaxiv.org/abs/2603.26233)
- Clarify before the expensive phase: deep research triages and clarifies first, then runs. — [The Decoder](https://the-decoder.com/openai-is-teaching-developers-how-to-build-deep-research-agents/)
- Defaults must be explicit, and only outcome-changing gaps go to the user. — brainstorming SKILL.md (superpowers 7.0.0, local copy)
- Claude Code pauses the loop for both permission and questions through one callback. Permission covers destructive actions, and AskUserQuestion covers ambiguity. — [Claude Agent SDK docs](https://code.claude.com/docs/en/agent-sdk/user-input)

### Inferences: a proposed policy and loop for PixPin
1. **Ground first (no question).** Before any ask, gather what can settle the ambiguity for free: the selected or open canvas or note, the chat history, recent captures, files. Many "ambiguous" requests are only ambiguous without that context.
2. **Gate (one cheap call).** A small model (Haiku, or the local model for offline use) returns JSON such as `{interpretations:[...], outcome_differs: bool, missing:[slot], cost_of_wrong: low|med|high, default: "..."}`. This follows the Intent-Sim and ClarifyGPT idea of comparing candidate readings rather than asking the model "are you sure?".
3. **Decide:**
   - Clear, or interpretations converge → act.
   - Ambiguous, but the cost of a wrong guess is low and it is easy to undo → act on the default, and begin the reply with one line such as "Asumí X; dime si era Y" ("I assumed X; tell me if you meant Y").
   - Ambiguous, and the cost is medium or high (long generation, overwriting content, sending or sharing, cloud cost) → call `ask_user`.
   - Destructive action → a separate confirmation, as with permissions, not a clarifying question.
4. **The `ask_user` tool.** It mirrors AskUserQuestion: 1 question per call by default (brainstorming), a hard limit of 2 to 3 options with short labels and descriptions, the recommended option first, "Otro…" ("Other…") free text always present, and an optional thumbnail preview for visual choices on the canvas. The loop suspends, and the answer comes back as the tool result.
5. **Budget.** At most one question round before the first visible result on normal requests. More rounds only for "project-sized" requests, where brainstorming-style playback applies. This respects the user's "latencia mínima" (minimal latency) rule. A cap is consistent with AskBench's 3-turn budget, but no study fixes the optimal number.
6. **Respect an opt-out.** If the user says "tú decide" or "no preguntes" ("you decide", "don't ask"), switch to act-and-state-assumptions for the rest of the session. This mirrors brainstorming's skip line.
7. **Evaluate with a bench.** A small set of PixPin requests, split into clear and ambiguous, should measure two things: the ask rate on clear requests (target near 0) and the ask rate on ambiguous ones (target high). This follows AskBench's IN3 split and the user's "iterar con el banco" rule ("iterate with the bench before compiling").

### Gaps
- No source gives a validated numeric confidence threshold for asking, such as an entropy cut-off. Thresholds would need tuning on PixPin's own bench.
- I found no evidence on how small local models perform as ambiguity gates. Every cited detector used GPT-4-class or 7B-plus models.
