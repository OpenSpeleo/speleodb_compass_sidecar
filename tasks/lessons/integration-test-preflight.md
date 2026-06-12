# Integration test preflight failures

When real integration-test credentials are explicitly configured, do not
silently skip tests just because the host is unreachable or authentication
fails. Fail the suite with a setup-focused preflight message, then let later
real-HTTP tests skip after that preflight failure is recorded so the output
does not look like many endpoint regressions.
