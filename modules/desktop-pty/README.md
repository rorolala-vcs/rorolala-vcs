# rola-desktop-pty

## Introduction

The pseudo-terminal behind the Desktop's Terminal dock: a shell run in a pty, reached over the C ABI as read, write, resize and close.

## Boundary

It owns the pty pair, the child process in it, the background drain into a queue, and the four exports a view calls. It does not parse terminal output or draw it: what the bytes mean belongs to the window that shows them, in `app/desktop`. The C# side of the same exchange is `modules/rola-sharp`. This crate starts a shell and carries bytes, and it holds no VCS logic.
