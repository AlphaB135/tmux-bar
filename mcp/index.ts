import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { execSync } from "child_process";

const server = new Server(
  { name: "tmux", version: "0.1.0" },
  { capabilities: { tools: {} } }
);

function run(cmd: string): string {
  try {
    return execSync(cmd, { timeout: 5000 }).toString().trim();
  } catch {
    return "";
  }
}

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "tmux_list",
      description:
        "List all tmux sessions. Shows session name, current command, window count, and attached status.",
      inputSchema: {
        type: "object" as const,
        properties: {},
      },
    },
    {
      name: "tmux_open",
      description:
        "Open a tmux session in iTerm2. Creates a new iTerm2 window and attaches to the session.",
      inputSchema: {
        type: "object" as const,
        properties: {
          session: {
            type: "string" as const,
            description: "tmux session name",
          },
        },
        required: ["session"],
      },
    },
    {
      name: "tmux_new",
      description:
        "Create a new tmux session with a given name. Returns the session name on success.",
      inputSchema: {
        type: "object" as const,
        properties: {
          name: {
            type: "string" as const,
            description: "Name for the new session",
          },
        },
        required: ["name"],
      },
    },
  ],
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name } = request.params;
  const args = request.params.arguments ?? {};

  if (name === "tmux_list") {
    const output = run(
      'tmux list-sessions -F "#{session_name}\\t#{session_windows}\\t#{session_attached}"'
    );
    if (!output) {
      return {
        content: [{ type: "text", text: "No tmux sessions running." }],
      };
    }

    const sessions = output.split("\n").map((line) => {
      const [sessionName, windows, attached] = line.split("\t");
      const cmd = run(
        `tmux display-message -t "${sessionName}" -p "#{pane_current_command}"`
      );
      const path = run(
        `tmux display-message -t "${sessionName}" -p "#{pane_current_path}"`
      );
      return {
        name: sessionName,
        command: cmd || "?",
        path: path || "?",
        windows: parseInt(windows),
        attached: attached === "1",
      };
    });

    return {
      content: [{ type: "text", text: JSON.stringify(sessions, null, 2) }],
    };
  }

  if (name === "tmux_open") {
    const session = args.session as string;
    if (!session) {
      return {
        content: [{ type: "text", text: "Error: session name required." }],
        isError: true,
      };
    }

    const check = run(`tmux has-session -t "${session}" 2>&1`);
    if (check && check.includes("not found")) {
      return {
        content: [{ type: "text", text: `Session "${session}" not found.` }],
        isError: true,
      };
    }

    const script = `tell application "iTerm2"
activate
create window with default profile
tell current session of current window
write text "tmux attach -t ${session}"
end tell
end tell`;

    run(`osascript -e '${script.replace(/'/g, "'\\''")}'`);
    return {
      content: [
        { type: "text", text: `Opened iTerm2 with tmux session: ${session}` },
      ],
    };
  }

  if (name === "tmux_new") {
    const sessionName = args.name as string;
    if (!sessionName) {
      return {
        content: [{ type: "text", text: "Error: session name required." }],
        isError: true,
      };
    }

    const result = run(`tmux new-session -d -s "${sessionName}" 2>&1`);
    if (result.includes("exist")) {
      return {
        content: [
          {
            type: "text",
            text: `Session "${sessionName}" already exists.`,
          },
        ],
        isError: true,
      };
    }

    return {
      content: [
        { type: "text", text: `Created tmux session: ${sessionName}` },
      ],
    };
  }

  return {
    content: [{ type: "text", text: `Unknown tool: ${name}` }],
    isError: true,
  };
});

const transport = new StdioServerTransport();
await server.connect(transport);
