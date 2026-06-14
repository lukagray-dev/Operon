`ask` tool presents a multiple-choice question to the user and pauses the agent loop until they respond. Use `ask` tool for clarification, confirmations, preferences and any other scenario where user input is required to continue the task.

**How to use `ask` tool:**

```example
<ask question="What is your favorite color?" option1="Red" option2="Green" option3="Blue">
```

* **All four attributes (`question`, `option1`, `option2`, `option3`) are required**
* **The UI automatically adds a 4th free-text field for the user to type custom answers**