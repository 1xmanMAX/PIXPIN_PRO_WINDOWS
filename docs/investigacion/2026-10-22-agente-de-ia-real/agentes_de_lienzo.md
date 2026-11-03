# How AI agents for whiteboards, canvases and diagram editors are built: canvas state, action spaces, image-to-canvas

## 1. tldraw (agent starter kit, make real): how shapes are shown to the model, which actions exist, coordinates, streaming, review

### Takeaway
tldraw's agent sends the model both a screenshot and simplified structured shape data. The data uses three levels of detail: Blurry for shapes in view, Focused for shapes being worked on, and Peripheral clusters for shapes off screen. It exposes many high-level layout actions (align, distribute, stack, place) and explicitly tells the model to prefer them over raw create/move. It makes coordinates chat-relative and rounded, applies actions as soon as each one finishes streaming, and runs a scheduled `review` pass over a region that relies mostly on the screenshot.

### Cited Findings
- The agent "captures both visual screenshots and structured shape data from the canvas." The structured data comes in three formats:
  - **BlurryShape**: shapes in the viewport, with bounds, id, type and text.
  - **FocusedShape**: shapes the agent is focusing on, with most of their properties. The model also outputs this format when it creates shapes.
  - **PeripheralShapeCluster**: off-screen shapes grouped into clusters, with bounds and a count.
  — [tldraw Agent starter kit docs](https://tldraw.dev/starter-kits/agent)
- The context the agent gathers includes:
  - the user's message and current selection
  - the user's viewport and any shapes, positions or areas they added
  - recent user actions and session history
  - a canvas screenshot
  - simplified viewport shapes and off-screen clusters
  - **"Lints flagging potential shape issues"**
  — [tldraw Agent starter kit docs](https://tldraw.dev/starter-kits/agent)
- The default actions are:
  - create, update and delete shapes
  - draw freehand pen strokes
  - rotate, resize, align, distribute, stack and reorder multiple shapes
  - write out reasoning and send messages
  - keep a todo list
  - move the viewport and count matching shapes
  - schedule follow-up work and call external APIs
  — [tldraw Agent starter kit docs](https://tldraw.dev/starter-kits/agent)
- The FocusedShape schema, as found in `shared/format/FocusedShape.ts`:
  - **geo**: `_type` (geo enum), `color`, `fill`, `x`, `y`, `w`, `h`, `note`, `shapeId`, optional `text`, optional `textAlign` (start/middle/end)
  - **line**: `x1`, `y1`, `x2`, `y2`
  - **text**: `anchor` (9 positions), optional `fontSize`, `maxWidth` (number or null), `text`, `x`, `y`
  - **arrow**: `fromId`/`toId` (shape id or null), `x1..y2`, optional `bend`, optional `text`
  - **note**: `x`, `y`, `text`
  - **draw**: cannot be created with "create"; it must come from pen events
  - **unknown**: carries `subType`; it can be moved but never created

  Every shape has a free-text `note` field in which the model explains the shape's purpose.
  — [FocusedShape.ts (GitHub raw)](https://raw.githubusercontent.com/tldraw/tldraw/main/templates/agent/shared/format/FocusedShape.ts)
- Coordinates: "every position sent to the model is offset by the starting position of the current chat" (helpers `applyOffsetToVec` and `removeOffsetFromVec`). Numbers are rounded with `roundAndSaveNumber` and restored with `unroundAndRestoreNumber`. — [tldraw Agent starter kit docs](https://tldraw.dev/starter-kits/agent)
- Streaming: "Shapes get created, updated, and deleted incrementally as each action finishes streaming." Each action checks `action.complete` before it is applied. — [tldraw Agent starter kit docs](https://tldraw.dev/starter-kits/agent)
- Sanitizers fix model mistakes before an action is applied: `ensureShapeIdExists`, `ensureShapeIdIsUnique`, `ensureValueIsVec`, `ensureValueIsNumber`. — [tldraw Agent starter kit docs](https://tldraw.dev/starter-kits/agent)
- Architecture: a client-side `PromptPartUtil` gathers data, and a worker-side `PromptPartDefinition` builds the messages (with `priority`; higher priority appears later). Actions are `AgentActionUtil` plus a schema with an `_type` field. "Modes" (`client/modes/AgentModeDefinitions.ts`) each declare `parts` (what the agent sees) and `actions` (what it can do). The system prompt is in `worker/prompt/buildSystemPrompt.ts`. — [tldraw/templates/agent (GitHub)](https://github.com/tldraw/tldraw/tree/main/templates/agent)
- Rules from the system prompt (`rules-section.ts`):
  - Coordinates: "0,0 is the top left corner", with y increasing downward.
  - Text: "The font size of a text shape is the height of the text". Default text is 26 px tall and about 18 px per character. "The easiest way to make sure text fits within an area is to set the `maxWidth` property."
  - Labels: 32 px of padding on each side, and a shape with a label is at least 100 tall.
  - Inner shapes must fit inside their container.
  - Arrows must be bound with `fromId`/`toId`. If an arrow bends the wrong way, invert `bend`. Don't create duplicate arrows.
  - Layout actions: "`distribute`, `stack`, `align`, and `place` are often better than `create`, `update`, and `move` because they are more efficient and accurate."
  - Review: use `review` with x/y/w/h for complex tasks, and not for simple ones. Check overlaps, disconnected arrows, labels outside their containers and floating shapes. If the review leads to changes, schedule another review. When reviewing, "rely **most** on the image provided."
  - Every action carries an `intent`.
  — [rules-section.ts (GitHub raw)](https://raw.githubusercontent.com/tldraw/tldraw/main/templates/agent/worker/prompt/sections/rules-section.ts)
- How make real works:
  - The selection is exported as an image and sent to GPT-4V, which returns HTML that is embedded as an iframe shape.
  - Iteration: the user annotates on top of the result and presses the button again.
  - An iframe shows up as an "empty rectangle" in image exports, so the previous HTML is sent as text alongside the image.
  — [tldraw blog: make real, the story so far](https://tldraw.dev/blog/make-real-the-story-so-far)
- A third-party write-up describes planning, working and review modes, where review mode removes most editing tools. This is not verified in the official README, which only shows an `interrupt()` into `'reviewing'` as an example. — [tldraw/templates/agent (GitHub)](https://github.com/tldraw/tldraw/tree/main/templates/agent); [search summary](https://tldraw.dev/docs/ai)

### Inferences
- PixPin's current summary (#ids, types, boxes, colors, texts) already matches BlurryShape. What is missing:
  - a detailed "focused" level only for the selection or the shapes being edited
  - clusters for off-screen shapes
  - **automatic lints** (overlaps, text overflowing its box, loose arrows) delivered in the context itself
- The `note` and `intent` fields per shape and per action are cheap and help the model (and the user) keep track of what each element is for.
- A screenshot plus structured data, with the instruction to "trust the image for quality and the data for ids", is the pattern tldraw has settled on.

### Gaps
- I could not open "tldraw computer" or the full current list of action schemas (`AgentActionSchemas.ts`). I also did not verify the internal wiring of `review` (whether it captures a cropped screenshot of the area).

## 2. Excalidraw AI, Mermaid-to-Excalidraw and DSL intermediate representations

### Takeaway
Excalidraw's text-to-diagram goes through Mermaid: the model writes Mermaid and the library converts it into native Excalidraw elements. There is direct evidence that intermediate plans and structured descriptions improve quality. DiagrammerGPT uses a plan with boxes, and Draw with Thought / VCG-Bench report clear gains. I found no paper that directly compares "layout engine vs coordinates chosen by the LLM".

### Cited Findings
- Excalidraw's "generate diagram with AI" feature generates MermaidJS and renders it behind the scenes. It is limited to a few generations per day on the free plan. This is a 2024 secondary source. — [smcleod.net, 2024](https://smcleod.net/2024/10/generating-diagrams-with-with-ai-/-llms/)
- `@excalidraw/mermaid-to-excalidraw`:
  - API: `parseMermaidToExcalidraw(diagramDefinition, config?)` returns `{ elements, files }`.
  - Config: `flowchart.curve` (`linear` or `basis`), `themeVariables.fontSize`, `maxEdges` (default 500), `maxTextSize` (default 50000).
  - Support varies by diagram type, and there is a guide for adding new types.
  — [excalidraw/mermaid-to-excalidraw (GitHub)](https://github.com/excalidraw/mermaid-to-excalidraw)
- Excalidraw MCP servers expose a "create_from_mermaid" tool so that Claude writes short Mermaid instead of Excalidraw JSON ("avoiding the cost of generating Excalidraw JSON directly"). — [glama: excalidraw-mcp-server create_from_mermaid](https://glama.ai/mcp/servers/debu-sinha/excalidraw-mcp-server/tools/create_from_mermaid); [mcp_excalidraw README](https://glama.ai/mcp/servers/@yctimlin/mcp_excalidraw/blob/381bc98bff5ef76225ac3e2284ba667d329a8770/README.md)
- DiagrammerGPT (COLM 2024):
  - Stage 1: an LLM (GPT-4) writes a "diagram plan" with entities, relations and **bounding boxes**, refined in a planner-auditor loop.
  - Stage 2: rendering, with the text labels drawn by a separate module.
  - It reports more accurate diagrams than text-to-image models and includes an analysis of vector output.
  — [arXiv 2310.12128](https://arxiv.org/html/2310.12128v2); [GitHub](https://github.com/aszala/DiagrammerGPT)
- MermaidSeqBench (NeurIPS 2025 workshop) evaluates natural language to Mermaid with an LLM-as-judge and finds large gaps between small models. It does not evaluate layout. — [arXiv 2511.14967](https://arxiv.org/html/2511.14967v1)

### Inferences
- For graph-type diagrams (flowcharts, sequences, ER, mind maps), the robust pattern is: the model emits a short DSL (Mermaid or a JSON of nodes and edges) → a deterministic layout engine places it → it is converted to native elements. This takes the coordinates away from the model, which is its weak point (see section 4). It matters most for DeepSeek and the local model.
- PixPin could expose a `diagram_from_graph(nodes, edges, direction)` tool with a layout engine on the Rust side, instead of asking for x/y.

### Gaps
- I did not verify how mermaid-to-excalidraw obtains positions internally (the docs page returned 404). I also did not check which Mermaid diagram types fall back to an image.
- I did not research Miro AI, FigJam AI, Figma Make, draw.io AI or Napkin (I ran out of time and search budget).
- I found no study that compares layout-engine output with LLM-chosen coordinates.

## 3. Image to diagram or vector (redrawing an image as an editable diagram)

### Takeaway
Current best practice is multi-stage. First, perceive and structure (groups, hierarchy, connections). Then generate code in parts (styles, nodes, layout, edges). Finally, refine against a validator. Frontier models (Gemini 3, Claude 4.5) produce valid code more than 90% of the time, but fidelity drops on harder diagrams, and **counting elements** is the main bottleneck. Small open models often fail completely.

### Cited Findings
- **VCG-Bench** (arXiv 2605.15677, 2026): raster image to mxGraph XML, with 1,449 diagrams from 6 domains.
  - Results:

    | Model | SCS | CodeXQA | ESR |
    |---|---|---|---|
    | Gemini-3-Pro | 0.779 | 0.931 | 0.963 |
    | Claude-4.5-Sonnet | 0.645 | 0.907 | 0.961 |
    | Claude-4.5-Opus | 0.650 | 0.895 | 0.925 |
    | GPT-5.2 | 0.562 | — | 0.842 |
    | GLM-4.6V (best open) | 0.343 | — | 0.747 |
    | Qwen3-VL-8B and 32B | 0 | — | 0 |

  - Counting is the main bottleneck. On counting questions, Qwen3-VL-8B scores 0.149, against 0.949 for Gemini-3-Pro. Wrong counts lead to wrong topology.
  - Ablation: removing the "structured caption" lowers SCS from 0.805 to 0.690 and CodeXQA from 0.930 to 0.786.
  — [VCG-Bench](https://arxiv.org/html/2605.15677v2)
- Critique: part of VCG-Bench's ground truth was generated with VLMs. — [pith.science](https://pith.science/paper/2605.15677)
- **Draw with Thought** (arXiv 2504.09479), training-free:
  - Stage I: coarse-to-fine planning (perceptual structuring, then semantic specification with regions, typed elements and layout constraints).
  - Stage II: XML generated in 5 parts (document, style, nodes, layout, edges), with multi-round refinement that stops when the draw.io verifier accepts the output.
  - With Claude 3.7 Sonnet on Hard: CLIP 0.60→0.70, FID 150→85.
  - Human evaluation of similarity: Claude 6.1, DwT 7.3, human reference 9.4.
  - Ablation on LLaMA-3.2-V-11B: XML validity is 89% with the full system, 73% without layout planning and 66% without hierarchical generation.
  - These results are self-reported on their own 247-diagram benchmark.
  — [arXiv 2504.09479](https://arxiv.org/html/2504.09479v1)
- figure-to-editable (a Codex skill): the agent does the image-to-scene step, and deterministic scripts do the export to draw.io/SVG. — [GitHub wangyunjeff/figure-to-editable](https://github.com/wangyunjeff/figure-to-editable)
- GenAI-DrawIO-Creator mentions image replication with Claude 3.7. — [arXiv 2601.05162](https://arxiv.org/html/2601.05162v1)

### Inferences
- For "redraw this image as a clean diagram" in PixPin, the evidence-backed pipeline is:
  1. The VLM writes a structured inventory (number of nodes, texts, groups, connections), with an explicit count.
  2. A graph DSL plus approximate boxes.
  3. Normalize and align with tools.
  4. Render, compare with the original side by side, and correct.

  Local OCR (PixPin already has OCR) can supply exact texts and boxes, which offsets the model's counting and reading errors.
- With the small local model this task will hardly work end to end (VCG-Bench: small open models score 0). Delegate it to Sonnet/Haiku.

### Gaps
- I did not review StarVector, AutomaTikZ, or classic OCR + shape-detection pipelines with quantitative results.

## 4. Lessons from GUI and computer-use agents (set-of-marks, grids, coordinate accuracy)

### Takeaway
VLMs are imprecise with raw coordinates. Labeling regions with numbered marks (Set-of-Mark), cropping and zooming, and controlling resolution help more than asking for exact numbers. Grids help or hurt depending on the model. With Claude, use absolute pixels on an image that has been resized beforehand.

### Cited Findings
- Set-of-Mark: regions are segmented with SAM/SEEM and overlaid with marks (alphanumeric labels, masks, boxes). Zero-shot GPT-4V with SoM outperforms a fully fine-tuned referring/segmentation model on RefCOCOg. — [arXiv 2310.11441](https://arxiv.org/abs/2310.11441)
- Claude coordinates:
  - "Claude works best with absolute pixel coordinates". Normalized coordinates (0–1000) work poorly.
  - Coordinates refer to the image after it is resized. The safest approach is to resize the image beforehand.
  - The tier limits:

    | Tier | Max long edge | Max visual tokens |
    |---|---|---|
    | Standard | 1568 px | 1568 |
    | High-resolution (Claude 4.7 and later) | 2576 px | 4784 |

  - For small elements, crop and send the crop (then offset by the crop's origin).
  - `"transformations": {"oversized_image": "error"}` makes the request fail instead of resizing silently.
  — [Claude docs: vision coordinates](https://platform.claude.com/docs/en/build-with-claude/vision-coordinates)
- Claude's documented limitations: spatial outputs are "approximate", counts of many small objects are imprecise, and images placed before the text work best. — [Claude docs: vision](https://platform.claude.com/docs/en/build-with-claude/vision)
- Grid overlays: in SketchVLM, a grid improves GPT-5's annotation precision but **hurts** Gemini-3-Pro. — [arXiv 2604.22875](https://arxiv.org/html/2604.22875)
- GUI-Primitives (2026): the strongest proprietary models reach only 24–31% on spatial primitives in screenshots. — [arXiv 2608.21832](https://arxiv.org/html/2608.21832v1)
- Coarse-to-fine cropping (GUI-Lens) improves grounding by up to 24.9 percentage points. — [arXiv 2608.03270](https://arxiv.org/html/2608.03270)
- R-VLM's zoom-in on region proposals adds 13% on ScreenSpot. — [arXiv 2507.05673](https://arxiv.org/pdf/2507.05673)

### Inferences
- PixPin's render for the model should draw **each element's #id on top of it** (Set-of-Mark), so the model refers to ids instead of coordinates. The screenshot should be resized beforehand to the tier limit, with a known scale.
- Offer a `zoom/review(x, y, w, h)` tool that returns a crop (as in tldraw) for checking small texts and overlaps.
- Do not add a grid by default. Its effect depends on the model, so test it on PixPin's own benchmark (Haiku vs Sonnet).

### Gaps
- I did not find data specific to Claude Haiku 4.5 or DeepSeek on coordinate accuracy. The local model was not covered either.

## 5. Layout helpers to expose as tools

### Takeaway
tldraw explicitly favors high-level actions (align, distribute, stack, place) over manual coordinates, and enforces text fitting through `maxWidth` and padding rules. The diagram pipelines delegate layout to an engine or verifier. The evidence points to giving the model declarative operations plus automatic lints.

### Cited Findings
- tldraw's actions include rotate, resize, align, distribute, stack and reorder. The prompt says they are "more efficient and accurate" than create/update/move. — [rules-section.ts](https://raw.githubusercontent.com/tldraw/tldraw/main/templates/agent/worker/prompt/sections/rules-section.ts); [tldraw Agent starter kit](https://tldraw.dev/starter-kits/agent)
- tldraw's text rules (`maxWidth`, 32 px padding, minimum label height of 100) and lints are part of the context. — [rules-section.ts](https://raw.githubusercontent.com/tldraw/tldraw/main/templates/agent/worker/prompt/sections/rules-section.ts); [tldraw Agent starter kit](https://tldraw.dev/starter-kits/agent)
- In Draw with Thought, removing layout planning lowers XML validity from 89% to 73%. — [arXiv 2504.09479](https://arxiv.org/html/2504.09479v1)
- Excalidraw MCP servers convert Mermaid with a deterministic engine instead of having the model write coordinates. — [glama create_from_mermaid](https://glama.ai/mcp/servers/debu-sinha/excalidraw-mcp-server/tools/create_from_mermaid)

### Inferences
- A minimum tool set for PixPin:
  - `align(ids, edge)`, `distribute(ids, axis, gap)`, `stack(ids, direction, gap)`, `place(id, relative_to, side, gap)`
  - `layout_graph(nodes, edges, direction)`, with a Rust layered engine in the style of dagre/ELK
  - `fit_text(id)`, which grows the box or wraps the text
  - `replace_text(id, text)`, which keeps the style and is useful for translating figures
  - `lint()`, which returns overlaps, overflowing text, loose arrows and elements outside their container
- "Translate text in a figure" barely needs coordinates: `replace_text` plus `fit_text` plus a lint pass.

### Gaps
- I found no published metrics comparing the quality of each specific helper (for example, ELK vs dagre in LLM agents). The recommendation of dagre/ELK is my inference, not a cited finding.
