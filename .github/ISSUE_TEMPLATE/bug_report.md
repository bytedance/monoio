---
name: Bug report
about: Create a report
title: ''
labels: A-monoio, B-bug
assignees: ''

---

**Version**
List the versions of all `monoio` crates you are using. The easiest way to get
this information is using `cargo tree` subcommand:

`cargo tree | grep monoio`

**Platform**
The output of `uname -a` and `ulimit -l`.

**Description**
Enter your issue details here.
One way to structure the description:

[short summary of the bug]

I tried this code:

[minimum code that reproduces the bug]

I expected to see this happen: [explanation]

Instead, this happened: [explanation]
