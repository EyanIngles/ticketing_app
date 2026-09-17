pub const TICKET_STATUSES: &[&str] = &[
    "queued",
    "running",
    "awaiting_you",
    "pr_opening",
    "pending_review",
    "closed",
    "failed",
    "open",
    "cancelled",
];

pub const ROLE_ENGINEER: &str = "Engineer";

pub const PERMISSION_PENDING: &str = "pending";
pub const PERMISSION_APPROVED: &str = "approved";
pub const PERMISSION_DENIED: &str = "denied";

pub const RELEASE_BIN_NAME: &str = "lyra-server";

pub const OPENCODE_DISPATCH_PROMPT: &str = "\
You are {agent_name}, role {agent_role} (type Agent).
Project: {project_name}
Project description: {project_description}
Ticket #{ticket_id}: {ticket_name}
Task: {ticket_description}

Do the work in this repo. Comment a short markdown summary via Lyra MCP, \
set ticket status awaiting_you. Do not open a PR until asked.";
