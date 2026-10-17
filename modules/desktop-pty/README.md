# rola-desktop-pty

A terminal's other half: the pseudo-terminal a shell is run in, across the C ABI.

The Desktop's Terminal dock draws what a shell prints and sends what a user types, and neither of
those is possible over a plain pipe: a program that is not talking to a terminal turns off its
prompt, its colours and its screen handling. A pseudo-terminal is the pair the system offers instead
— the shell gets a terminal, this side gets a byte stream — and what this crate adds is that pair,
the process in it, and the four things a view does with it: read, write, resize and close.

Only the bytes cross. What a terminal *means* is a parser and a drawing, and both belong to the
window that shows them; what is here is the exchange.
