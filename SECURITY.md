# Security policy

Report vulnerabilities privately through GitHub's "Report a vulnerability"
button on the repository (Security tab). Please do not open public issues for
security problems. We aim to acknowledge reports within 7 days.

Scope: memory safety of the two `unsafe` modules (`draw_buf::alloc`,
`draw_sw::blend::words`), panics reachable from untrusted inputs (fonts,
images, text), and unbounded allocation from untrusted inputs.
