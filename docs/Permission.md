# Permission Model

Operon is built to talk to anyone — your customers on WhatsApp, your team on Telegram, or just you from your terminal or app. That openness is the whole point. But it immediately raises a question:

> ***If anyone can message Operon, what can Operon do on their behalf?***  
> The answer is: exactly what you decided in advance. Nothing more.

## The Core Idea

When Operon receives a prompt, the first thing it does is, figure out who sent it.

Every sender is classified as one of two roles:

- **Owner:** you, your staff, people you trust completely.
- **External:** customers, leads, patients, the public. Anyone else.

This classification happens at the channel level. A message coming from your own terminal is Owner. A message arriving through a public WhatsApp number is External, unless you explicitly mark that contact as trusted.

Once the role is known, Operon looks up what it's allowed to do for that role. If the permission isn't explicitly granted, the answer is no.

## Tools and What They Touch

Operon's capabilities come from **tools**. Discrete actions it can take, like reading a file, running a shell command, or searching the web. Not all tools are equal, and not all of them touch the same things.

This splits tools into two categories:

1. **Global Tools:**  
These tools don't touch your file system at all. They work the same regardless of what directories you've added. You set their permissions once, globally, per role.

| Tool | What it does |
|---|---|
| Web | Search the web and fetch URLs |
| Sub-agents | Spin up child agents to handle subtasks |
| Ask Question | Ask the user a clarifying question |
| Task Management | Create, track, and manage ongoing tasks |
| Load Tools | Load available toos |

2. **Directory-Scoped Tools:**  
These tools touch your actual files and system. Their permissions are tied to specific directories. And each directory can have completely different rules.

| Tool Group | What it does |
|---|---|
| File System | Read, write, list, create, and delete files |
| Shell | Run commands and scripts |

You decide which directories Operon can even see. Anything outside an added directory is completely inaccessible. Operon can't read it, write to it, or acknowledge it exists.

## How Directory Permissions Work

Say you add two directories:

```
~/work/client-project
~/personal/notes
```

For `~/work/client-project`, you might configure:

| Tool | Owner | External |
|---|---|---|
| File System | Allow | Ask |
| Shell | Allow | Deny |

For `~/personal/notes`:

| Tool | Owner | External |
|---|---|---|
| File System | Allow | Deny |
| Shell | Deny | Deny |

Now when a customer messages Operon asking it to "check the project files" — Operon can look inside `~/work/client-project`, but only with your confirmation (Ask). It cannot touch `~/personal/notes` at all. It cannot run any shell commands for them.

When *you* send the same request, Operon has full access to `~/work/client-project` and can run commands there freely.

Same agent. Same prompt. Different role → different outcome.

## The Three Permission Modes

Every tool permission is set to one of three modes:

| Mode | Meaning |
|---|---|
| **Allow** | Operon can use this tool freely for this role |
| **Ask** | Operon must ask for confirmation before using this tool |
| **Deny** | Operon cannot use this tool for this role, period |

**Ask** is the important middle ground. It lets you open up access to external users without giving them unsupervised control — Operon will pause and check with you before acting.

## Tool Groups and Individual Overrides

In the UI, tools are organized into groups (e.g. *File System*, *Shell*, *Web*). Setting a group's permission applies to all tools in that group at once.

If you need finer control, you can expand a group and set permissions per individual tool. The moment individual tools within a group differ from each other, the group-level display shows **Custom** — a signal that the group has mixed rules.

```example
> File System    Custom    Ask
  ├ read_file    Allow     Allow
  ├ write_file   Allow     Ask
  ├ list_dir     Allow     Allow
  ├ create_dir   Allow     Deny
  └ delete_file  Ask       Deny
```

## Why This Matters

Most agent tools were built for a single user — the developer running them locally. Permissions weren't a priority because there was only ever one person involved.

Operon is built for deployment. You might be a doctor letting patients book appointments over WhatsApp. A business owner letting leads ask product questions over Telegram. A freelancer giving clients a way to check project status without calling you.

In every one of these cases, your external users need *some* access — but absolutely not *all* access. The line between "can ask about availability" and "can read my entire file system" needs to be hard, explicit, and yours to draw.

That's what this permission model does. External users get zero access by default. You define exactly what they can reach, in which directories, using which tools, and whether Operon needs your confirmation first.

Access is segmented by design. Not by hope.

---

### Summary

```
Owner sends prompt
  → check Owner permissions per tool (global + per directory)
  → act within those bounds

External user sends prompt
  → check External permissions per tool (global + per directory)
  → act within those bounds
  → for Ask-mode tools: pause and request owner confirmation first
  → for Deny-mode tools: refuse entirely, never expose why
```
