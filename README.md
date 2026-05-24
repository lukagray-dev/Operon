<div align="center">

# **Operon**

***The autonomous AI agent built for everyone — not just developers.***

![Operon](assets/logo.svg)

<br/>

> *Claude Code, Codex, and OpenClaw are powerful — but they're built for engineers who live in a terminal.*  
> ***Operon is built for everyone.***

</div>

## What is Operon?

Operon is a **consumer-first** AI agent similar to OpenClaw but with a clean GUI. It does everything that OpenClaw do — but without requiring you to know what a terminal is.

> **You open Operon → type what you need.** *That's it. ✓*

- Underneath, *Operon* runs a production-grade Rust agent runtime similar to Codex/OpenClaw.
- The difference is the surface: instead of a terminal or an IDE, Operon gives you a familiar chat interface.

> Think ChatGPT app, but with the full autonomous capability of an agent.

## 🐦‍🔥 Back Story

Hi, I'm **Luka Gray** (aka Soumo Mukherjee).

When I first used OpenClaw, one thing became obvious: the intelligence was impressive, the user experience was... a crime scene.

So in early 2026, I started building **Operon**. 🎉

**The mission is simple**: Build powerful AI agents that our *granny* can use, while keeping the depth developers expect.

> ***Built for normal people, because software has ignored them for long enough.***

<details>
<summary><span style="font-size: 1.5em;">⚡ Features</span></summary>
<br>

1. 🗣️ **Chat-First Interface**
   - Operon works through a ChatGPT-like interface, because billions already know how messaging works.
   - Also available in **TUI**, **VS Code** (in development), **JetBrains** (in development), and **Mobile**.
2. ⚡ **Lightweight by Design**
   - Operon’s backend is written in Rust, because wasting RAM with Node.js became strangely normal.
3. 📱 **Mobile-Ready Architecture**
   - Built to run beyond desktops with shared core runtime and portable frontends.
4. 🔌 **Multi-Provider LLM Support**
   - Use OpenAI, Anthropic, local models, OpenAI-compatible APIs, and more.
5. 📡 **Connector Channels**
   - Connect through WhatsApp, Telegram, Gmail, and other external channels.
   - Work with your agent remotely.
6. 🌐 **Multi-Format Patch Engine**
   - Supports Codex patches, unified diffs, and SEARCH/REPLACE blocks.
   - Because models disagree on formatting with remarkable confidence.
7. 📋 **Tasks & Memory**
    - Operon remembers what it was doing, unlike most meetings.
    - It keeps structured memory across sessions, tracks ongoing tasks, supports scheduled actions, and brings relevant context forward so you don’t have to repeat yourself every morning.

</details>

## 🛡️ **Permission Model (Here Operon Becomes Useful for Real Business)**

Most AI agent tools were built for developers talking to themselves. That works fine for devs but not for business.

1. **If you're a Doctor**:
    - You want patients to book appointments, ask timings, and receive follow-ups, but doing all these manually feels frustating on daily basis.
    - So, you had to hire an assistant for that. But that requires you to pay his salary.
2. **If you're a business owner**:
    - You want leads to ask product questions, request quotes, and book services, but you do **not** want every WhatsApp/Telegram stranger getting owner-level access because software had a lazy afternoon.

> This is exactly where **Operon** is different.

Every interaction is classified as:

- **Owner** → you, staff, trusted people  
- **External** → customers, leads, public users, and other adventurous strangers

External users get **zero access by default**.

Only you decide what they can use:

$$
\text{Per tool → Per directory → Per connector channel}
$$

### Real Examples

- A patient can book appointments on WhatsApp, but cannot access anything else.
- A customer can ask pricing and availability, but cannot touch local files.
- A staff member can manage one folder, but not your full system.
- A lead can message your agent at midnight while you sleep, because exhaustion is not a business strategy.

### Why This Matters?

Some tools like **OpenClaw** rely on a simpler allowlist model. That sounds fine until you open public access. (Everyone is treated as owner)

Because once everyone is treated like a trusted user, you are trusting strangers on the internet to behave responsibly, which has never been a winning business model.  
Convenient, in the same way leaving your clinic unlocked is convenient.

🚨 **That creates obvious risks**:

- **Prompt injection** → users try to manipulate the agent into ignoring instructions.
- **Data leakage** → internal notes, files, customer data, or private context can be exposed.
- **Tool abuse** → outsiders triggering actions they were never meant to access.
- **System compromise** → broad permissions turn small mistakes into expensive ones.
- **Operational chaos** → one bad prompt can waste time, money, or trust.

---

> > ✅ **Operon prevents this by separating public users from trusted owners at the permission layer itself.**  
> > You define permissions **globally, per directory**.

That means:

- Public users can interact safely with limited tools only  
- Sensitive folders remain blocked unless explicitly trusted  
- Internal workflows keep full power without exposing the system  
- One bad prompt cannot magically inherit global access  

Access is segmented by design, not by hope.  
They do **not** get a backstage pass to your business.

***Built for trust, not just capability.***

---

## 🧠 OHub — Skills & Extensions Marketplace

OHub is Operon’s built-in marketplace for skills, extensions, and integrations.

Install new capabilities when needed, from business workflows to external services. Packages are verified before installation, because blindly trusting downloads has always been a bold strategy.

---

## Roadmap

```readmap
Desktop v1
↓
Mobile
↓
VSCode
↓
JetBrains
↓
Enterprise deployment
```

## Getting Started

> **Operon is currently in active development. Pre-built binaries are not yet available.**

---

### Contributing

Contributions are welcome. If you're planning a large feature or architectural change, open an issue first so we can avoid two people solving the same problem in different and equally dramatic ways.

For bug reports, include:

- OS / distro  
- Rust version  
- Operon version / commit hash  
- Model provider used  
- Logs or error output  
- Minimal reproduction steps  

The more precise the report, the faster the fix. “It broke” is emotionally honest, technically useless.

---

### License

Operon is licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**. See [LICENSE](./LICENSE) for full terms.

---

<div align="center">

<br/>

Built by **Soumo Mukherjee (aka Luka Gray)** • West Bengal, India • 2026  
*"The best tools disappear. You stop thinking about the tool and start thinking about the work."*

<br/>

**[GitHub](https://github.com/lukagray-dev) • [Instagram](https://www.instagram.com/lukagray.official) • [Email](mailto:heylukagray@gmail.com)**

<br/>

</div>
