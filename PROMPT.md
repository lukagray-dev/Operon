I was working on this project, more details here, `D:\Operon\README.md` and here, `D:\Operon\docs`.

---

Rightnow, I'm working on the VS Code extention. I already built the extension but rightnow it's totally same as the gui. we need some practical changes that will be only vscode specific.

---

* When we open the VS Code IDE and if there is no project folder opened, our extension will not start to work. If the user opens our extension, then it will tell the user to open a project.

Why is that? let me give you some context:  
In the GUI app we let the user open projects from the titlebar's `File` menu. And when the user opens a project specific chat session, we change operon's workspace path details in the `operon-snapshot` crate.  
By default the snapshot crate refers to `~/.operon/workspace/` as it's default workspace - all general chat sessions use that default workspace.  And when the user opens a project and if that is not present in the allowed directories -we automatically add that in allowed directories - so that the directory scoped permission layer can get satisfied.  

Now, when we are in an ide and no project is opened, there is no project directory which the `operon-snapshot` crate can refer and can be added in the allowed directory. Also, we can't let the agent to work on the default directory too.  

So, we'll enforce this logic in the extenside side, that if no project is opened in the ide that operon will not show it's interface. It will show a disclaimer-like page where the instructions will be written to open a project in the ide.  
And when the user opens a project in the ide, operon will remove that disclaimer-like page and show it's interface.  

In the sidebar of operon, we currently show general chats and project's chats both (same as gui). We need to modefy that too.  
We'll only show the project specific chats there + right now clicking on the `New conversation` button in the top of the sidebar creates a general chat session - we'll change it to create a project specific chat session (of the currently opened project in the ide).  
Also, when the user opens a project folder in the ide, we need add that directory in allowed directories too.  

> All of these will be only vscode extension specific. the core `operon-rs` or other parts (`gui` or `tui`) won't be affected.  

> Go ahead and explore the codebase properly to gain proper context then write a robust implementation plan.