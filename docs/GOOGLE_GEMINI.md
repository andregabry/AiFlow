# Google Gemini adapter

Official CLI: `gemini`. The adapter requires advertised `--prompt` and `--output-format stream-json`; the task body is supplied via stdin. Resume requires both advertised `--resume` and a session ID captured from official output.

This implementation does not assume an official noninteractive account-status command. Authentication is UNKNOWN until checked by the official CLI at execution time. Use Gemini CLI's interactive official login/logout; AiFlow does not read or copy its credential files and does not fabricate a Connected status. AiFlow identity and Gemini authentication remain separate.

Gemini was not found during local environment inspection. Live streaming, account entitlement, sandbox behavior and quotas require validation on a machine with the official CLI installed. Adapter defaults do not enable automatic approval.

Reference: [official headless reference](https://geminicli.com/docs/cli/headless/).
