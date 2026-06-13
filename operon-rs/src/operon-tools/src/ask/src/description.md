Presents a multiple-choice question to the user and pauses the agent loop until they respond.

Format:
<ask>
<<<<
question="[question_text]"
option1="[first_option]"
option2="[second_option]"
option3="[third_option]"
>>>>

Constraints & Usage:
- All four body options (`question`, `option1`, `option2`, `option3`) are required.
- The UI automatically adds a 4th free-text field for the user to type custom answers.
- Execution stops and waits until the user selects or inputs an answer.
