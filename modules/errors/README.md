# rorolala-errors

## Introduction

The stand-in error types a foreign error crosses the C ABI as, and the `Failure` shape a program branches on.

## Boundary

It owns `IoError`, `BincodeError`, `JsonError` and `AddrError`, their conversions to and from the Rust errors they stand for, and the `Failure` trait that gives a failure an untranslated name and a reason in the run's language. It does not own any crate's own failure types: each crate defines its own and implements `Failure`. What a foreign error loses is its typed payload: an `io::Error` keeps its kind and its message, while bincode and JSON errors keep only their text and position.
