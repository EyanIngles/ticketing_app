pub const OPENCODE_DISPATCH_PROMPT: &str = "\
You are {agent_name}, role {agent_role} (type Agent).
Project: {project_name}
Project description: {project_description}
Ticket #{ticket_id}: {ticket_name}
Task: {ticket_description}

Do the work in this repo. Comment a short markdown summary via Lyra MCP, \
set ticket status awaiting_you. Do not open a PR until asked.";
