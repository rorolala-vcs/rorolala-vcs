---
title: "Article Example"
description: "A sample Rorolala Book article for demonstration"
keywords: example, template
---

# ROLA BOOK

This article explains how to write ROLA books. Books can be queried and read in the GUI, CLI, and Web.

When writing a ROLA book, you don't need to use Markdown format—just place md files directly into the `book/` directory.
Every md file needs to include the following metadata at the top:

```yaml
---
title: "Article Example"
description: "A sample Rorolala Book article for demonstration"
keywords: example, template
---
```

Please note that book content **does not support images or HTML**; such content will not be recognized, so it's recommended to write using plain text only.

After writing or modifying, run the following command to recompile the book into `build/docs/builtin.book`:

```bash
cargo dev refresh-book
```
