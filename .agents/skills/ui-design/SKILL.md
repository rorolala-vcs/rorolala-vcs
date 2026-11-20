---
name: ui-design
description: Load this file when you need to design a graphical interface for RorolalaDesktop. It explains how to design interfaces that meet the requirements.
---

# Interface Design

RorolalaDesktop is a tool, not a show. An interface earns trust by being fast, quiet, and predictable. Every choice below serves that.

## Start from the current state

Before drawing anything, look at what the desktop program looks like now. A new interface should feel like it belongs to the same product, not like a foreign add-on.

Before you begin, determine the states the interface must cover: empty, loading, with data, error, disabled, selected, hover, focus. A design that only shows the happy path is incomplete.

## Replace text with interface

Text is the most expensive way to explain an interface. Users read labels only when necessary, and lengthy explanations slow down every visit.

Prefer layout, grouping, affordance, icons, states, and motion over long descriptions. A button labeled `Save` does not need a paragraph above it. A switch that clearly changes state does not need a caption repeating that state.

Use text only for what the interface cannot express: names, values, timestamps, errors, and the consequences of destructive actions. When a sentence explains how to use an obvious control, delete it and fix the control instead.

Error and empty states should say what happened and what the user can do next. No apologies, no narration of internal details, no encouraging paragraphs.

Tooltips supplement labels, they do not replace them. Avoid tooltips that repeat visible text.

## Use standard colors

The desktop program has a palette. Take colors from it; never invent colors.

- When semantic roles already exist (for example surface, on surface, accent, danger, muted), use those roles.
- Check the full state set: default, hover, pressed, selected, disabled, focus. A color that only works in the default state is not finished.
- Verify contrast under every theme the app ships. A subtle border that disappears in light mode is a defect.
- Do not introduce a new accent color for a single interface. If the palette genuinely lacks a role, discuss it with the user before adding one.

When a prototype needs colors, approximate the standard palette and note in the prototype that the colors are not final.

## Motion must not slow users down

Motion exists to explain state changes, confirm actions, or preserve spatial continuity. It never makes users wait, and it never competes with the task for attention.

Duration budgets:

- Immediate feedback, such as hover, press, and focus: 100 to 150 ms.
- State transitions, such as expanding, selecting, and switching tabs: 150 to 250 ms.
- Panel enter and exit: 200 to 300 ms.
- Transient elements may disappear faster than they appear.

Use standard easing curves. Linear motion feels mechanical. Spring and bounce curves draw attention to the animation itself rather than to the result.

Animate cheap properties by default, and avoid properties that trigger layout calculation, since those show up as jank on long lists. When a panel must change layout, accept that cost and keep the change brief.

Motion must be interruptible. A user who moves in and out quickly, or switches tabs mid-transition, should see the interface follow their input rather than a sequence of animations playing out in order.

Do not block input while animating. A control that ignores clicks for 300 ms because it is busy animating feels broken.

Respect the "reduce motion" setting. Read the platform setting when available and provide an in-app preference. "Reduce motion" keeps short opacity fades and removes translation, scaling, and parallax. "Off" sets durations to zero. Centralize durations, easing, and travel distances as tokens so this switch lives in one place rather than being scattered across styles.

No decorative motion. No idle pulsing, no floating, no confetti, nothing that exists only to "feel alive."

## When the answer is uncertain, prototype in the browser first

Desktop interfaces iterate more slowly than the browser. When layout, density, hierarchy, or motion timing is unclear, build a small web prototype first, show it to the user, then move to the real implementation.

Choose the location by phase:

- Planning phase: write to `__PLAN_<name>/__prototype/index.html`.
- Implementation phase: write to `__ui_prototype_<name>/index.html` in the project root.

Replace `<name>` with a short kebab-case label describing the interface. Temporary paths start with `__` and stay inside the project root; the user deletes them once the design is settled.

A prototype is a question, not a deliverable. It should contain:

- Real content shapes, using realistic sample data rather than placeholder lorem ipsum.
- Key states: empty, loading, error, disabled, selected.
- The interaction under discussion, clickable enough to feel the flow.
- The intended motion, using real durations.
- A note on the page stating that it is a prototype and the colors are approximate.

Keep it to a single HTML file with inline CSS and scripts. Do not wire it into the build output or production assets.

When you ask the user to review, say which decision the prototype is meant to settle: layout hierarchy, information density, transition duration, or the position of a particular control. When the answer is a multiple choice, give concrete options. Do not just send a link and say "what do you think?"

Once the user approves the direction, port it to native controls and standard assets. The prototype settles intent, not implementation. Do not copy browser-specific layout hacks into the real interface.

## Before delivery

- The interface uses standard color resources, with no literal color values.
- Every state the interface can enter has a design.
- Text is limited to names, values, errors, and consequences.
- Motion is within budget, animates only cheap properties, is interruptible, and respects "reduce motion."
- Keyboard focus is visible, and primary actions are reachable without a mouse.
- The result is consistent with existing patterns in the desktop program.
- Any prototype used to settle the design has been reviewed by the user, and its temporary directory has been deleted as agreed or left for the user to delete.
