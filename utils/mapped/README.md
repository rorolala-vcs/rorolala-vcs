# rorolala-utils-mapped

## Introduction

Reading a file through a memory mapping where that is cheaper than reading it into a buffer, and a run of bytes that is mapped ranges and owned buffers laid end to end.

## Boundary

It owns the mapping, the run, and the sizes at which a mapping stops being worth more than a read. It does not know what the bytes are: no store layout, no frame, no codec, and no file format.
