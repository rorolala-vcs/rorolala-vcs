---
name: write-documents
description: Load this skill when you need to write documentation or similar material, to learn how to produce docs that meet the standard
---

The project directory `.agents/skills/write-documents/langs/` holds versions of this skill in other languages, one file per language (for example `.agents/skills/write-documents/langs/zh_CN.md`). If the document you are writing is in one of those languages, open that file and follow it as well.

# How to Write Technical Documentation: Decide First, Then Write

Before you write, decide where the reader stands, what this sentence does for him, and what he loses if you delete it. Format follows from that decision; make the decision first, then pick the format.

## Three Questions to Answer Before You Write

1. **Who is the reader, and what does he already know.** Assume an experienced developer who can set up an environment and use common tools. He needs the facts, constraints, and paths for this project; introductory programming knowledge is a given. Use this test when deciding whether content stays: without it, can he still complete the task?
2. **What does he want to do when he reads this.** Documentation usually serves one action: locate, execute, troubleshoot, look up a parameter. When a passage serves two actions, the reader has to switch attention. Decide which action this passage supports, then decide what to write.
3. **What does he lose if you delete this sentence.** If he loses a condition, a result, or a risk, keep it. If he loses nothing, delete it. Repeated premises, filler transitions, and explanations of common knowledge usually fall into the second case.

## Tone

Write in a professional, clear, practical, and friendly tone. Aim for a formal project README for developers and technical creators: state the product's value with confidence and restraint, put facts first, and lower the barrier to use.

- **Use the second person and imperative mood.** Address the reader as "you," and guide steps with imperatives such as "install the dependencies" and "run the command below." Keep the structure clear, and use headings, lists, code blocks, and bold text to surface key concepts and flow.
- **Be instructive, concise, and direct.** Use terms precisely; explain design rationale when needed, and no more than that.
- **Be positive but objective.** Be confident in what the project can do, and don't promise what it can't.
- **Let explanation carry the information.** Keep adjectives only when they don't affect judgment; cut long lyrical passages, marketing hyperbole, and emotional language.

## Stance

- **State facts, conditions, and results, and let the reader judge.** Don't evaluate the reader's level or choices, and don't reach conclusions for him with "obviously" or "it's very simple." If there is a constraint, state it and why; if there isn't, don't manufacture tension with "must" or "must not."
- **State things directly.** A negation-then-affirmation sentence makes the reader discard a wrong answer before he gets the right one; "A is this, B is that, and it lands in two steps via…," "not X but Y," and "no, it's just…" all belong to this pattern. So do definition lists of the form "X is…" and setups of the form "through a series of…." Write the thing you mean to say.
- **Be honest with the reader.** Mark the scope of anything uncertain: "in the current version," "on this platform." If something can't be done, will fail, or has a cost, say so. When the reader needs to look something up elsewhere, give a link or a pointer instead of restating it here.
- **Keep the author's judgment, and control the emotion.** The author's presence comes from judgment and tone. A little "generally speaking," "of course," or "there's a pitfall here" gives the writing warmth; exclamation marks, rhetorical questions, and exaggerated adjectives make the reader remember the attitude instead of the information. Formal content (contribution guidelines, license notes) uses a standard, formal register without a personal voice.
- **Density comes from deleting sentences that carry no information.** Density comes from every sentence adding something new; shortening sentences doesn't add density. Delete sentences that carry no information and density rises on its own; drop a condition, an object, or a result just to compress, and the reader has to go back and guess. It's better to spend one more sentence making the premise clear than to trade a nice line for ambiguity.

## Get the Structure Right

- **Each paragraph advances one thing.** Intent decides where a paragraph ends; length is not the criterion. After reading a paragraph, the reader should be able to say what it told him. If a paragraph locates, operates, and explains at the same time, split it; if several sentences all say the same thing, merge them. Put a blank line between paragraphs so the boundaries of intent are visible; the blank line isn't there for decoration.
- **Separate action from reason.** Write execution steps and mechanism explanations separately. Give the action first and then explain why, or give the reason first and then the action. Either order works; interleaving them doesn't. Keep warnings in their own paragraph or in an admonition, never in the middle of a step.
- **Put examples where the reader acts.** Use commands he can copy and outputs he can observe. Order examples by the most likely sequence of use: default usage first, explicit parameters and edge cases after. Don't restate in prose what an example already shows; when the example ends, stop—no summary, no comparison table, no "in summary." Without examples, close the section without introducing anything new.
- **Let form follow relationship.** Layout makes relationships visible; determine the relationship first, then choose a list, table, or diagram.

  | Relationship between content       | Usual form                 |
  | ---------------------------------- | -------------------------- |
  | Parallel items, steps, check items | List; number it if ordered |
  | Lookup, comparison                 | Table                      |
  | Sequence, dependency, hierarchy    | Diagram or numbering       |
  | Content to be copied verbatim      | Code block                 |

  Tables answer "what is this value" and aren't suited to carrying reasoning; write a cell up to "what it is" and no tutorial. Platform differences must land on concrete values; when comparing several systems, a table is usually clearer than a sentence. Diagram node names match the body text; when one sentence can state the relationship, skip the diagram.

- **A heading is a promise.** Write as much body text as the heading promises. Use plain, descriptive nouns and avoid vague labels like "Introduction" or "A Brief Look" whose scope can't be judged.

## Sentences and Word Choice

- **One sentence advances one thing.** Make the subject and the action clear, and write conditions, actions, and results in the order they happen. Split sentences whose stacked modifiers force the reader to go back. Use connectives when they help: because, so, but, if, first, then, finally; they make relationships visible and aren't clutter.
- **Speak with observable facts.** Prefer what the reader can verify: commands, paths, exit codes, output, version numbers. Use general words only when judgment doesn't suffer. Write "fails" as "exits with a non-zero code"; rewrite "configure it" as the key and value to change. A description the reader can follow is more useful than one that sounds smooth.
- **Keep words stable, and justify degree words.** Use the same name for the same object, and say what it is the first time it appears. Keep degree words only when they convey a constraint: "any" and "must" carry information; "very simple" and "just" usually don't. You may bold key conditions, constraints, or conclusions—at most once per paragraph, marking the word rather than the whole sentence.
- **Use a well-established abbreviation when one exists.** When a term has a widely recognized short form, write the short form rather than the full word: i18n for internationalization, l10n for localization, and so on. Write the full word at first mention if the reader might not know it, then use the abbreviation from there on. Don't invent abbreviations the reader has to decode.
- **Person can be natural.** Address the reader as "you," or keep an impersonal statement. Pick whichever reads better; don't avoid the second person just to sound objective.

## When to Include Warnings

Warnings manage risk. Include one only when the reader might fail, lose data, waste a lot of time, or when platform, version, order, or dependency differs.

Form follows risk:

| Type of risk                    | Form          |
| ------------------------------- | ------------- |
| A single fact                   | Body text     |
| A key constraint                | One bold word |
| Pitfall, shortcut, special case | Admonition    |

Bold text and admonitions don't repeat what the body already says.

Skip warnings for basics the reader already knows, obvious consequences, and things the body already explains. Say what will happen.

Before you write a command, confirm it runs in the reader's directory, permissions, shell, and platform.

## Check with Questions When You're Done

- If the reader read only the headings, code blocks, and tables, could he complete the task?
- Delete each paragraph: what would the reader lose? If you can't say, delete it.
- Which sentences only explain what you just said?
- Which sentences use definition lists of the "A is this, B is that, and it lands via…" kind, or the negation-then-affirmation pattern? Rewrite them as direct statements.
- Where does the reader have to go back to find a premise? Move the premise forward.
- Which full words have a widely recognized abbreviation, and which abbreviations are used before they're introduced?
- After the example ends, does the reader need one more sentence from me? If so, add it to the example; if not, stop.
- Does each warning sit at the step that will actually fail? Does it repeat the body text?
- Does the body deliver the full scope the heading promises?

## Common Imbalances

| Symptom                                           | Underlying problem                               | Fix                                                             |
| ------------------------------------------------- | ------------------------------------------------ | --------------------------------------------------------------- |
| Long, and the reader still can't act              | Explaining the system, not supporting an action  | Write the action path first, then the reason as needed          |
| Every paragraph is right, but they don't flow     | Paragraphs hold parallel facts without advancing | Reorder by the reader's sequence of actions                     |
| It reads like a template                          | Sentences fill space instead of answering        | Ask of each sentence: what is lost if it goes?                  |
| Many warnings, and the reader still errs          | Warnings aren't at the risk point                | Move the warning to the step that fails                         |
| Lots of formatting, and the point is still buried | Form doesn't match the relationship              | Determine the relationship, then pick a list, table, or diagram |

## In One Sentence

Decide what the reader needs to do right now, then decide whether each sentence, paragraph, and format should exist. State the facts plainly, delete what carries no information, warn only where something will break, and keep the tone at the surface.

---

This file is a basis for judgment; use it with your own judgment, and write by judgment rather than copying templates. The rules come from limited comparison—fix them when you find counterexamples; direct notes from the author on a draft take precedence over this file.
