# Feature Specification: Image Prompt

**Feature Branch**: `005-image-prompt`

**Created**: 2026-11-25

**Status**: Draft

**Depends On**: `.specify/specs/003-integrated-matrix-bot/spec.md` (triggered prompts are evaluated only when `BOT_ENABLED=true`)

**Input**: User description: "When someone sends an image from a Matrix client such as Element in a message that should reach the model, the server passes that image to the model evaluating the prompt. The first version handles one image file. If the model cannot process images, or the server cannot tell that it can, the server sends an error back to the room and does not pretend the prompt was evaluated. Other file types are out of scope."

## Clarifications

### Session 2026-11-25

- Q: What is the first attachment the agent accepts? → A: One image file attached to a prompt. Other file types are out of scope.
- Q: What happens when the model cannot process images? → A: The room receives an error. The prompt is not evaluated. The failure is not silent.
- Q: What happens when the server cannot tell whether the model can process images? → A: The same room error. The server does not guess.
- Q: Does the service that evaluates prompts need a new feature to receive an image? → A: No. That service already accepts one image together with the text. This feature sends the image on the evaluation this server already makes. The fleet that runs the evaluator does not need a change.
- Q: What if the image cannot be included with the prompt? → A: The room receives an error, and the prompt is not sent. The evaluator will answer the text alone when the image is missing or rejected, so this server must refuse before that call.
- Q: If the image is sent to the model and the model service then fails to answer, what should the room show? → A: Post one room error saying the model did not answer.
- Q: How many images does one message carry in this version? → A: One image per message. A second message with its own image is a separate prompt. One message that itself contains more than one image still gets the one-image error. Element sends one image per message, so that error is an automated check, not an Element lab step.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Refuse an image the model cannot see (Priority: P1)

A person addresses the agent from Element with one image and a caption. The model that evaluates prompts cannot process images, or the operator has not said that it can. The person sees one error in the room. The caption is not sent for evaluation. A later text prompt, with no image, still receives a normal answer.

**Why this priority**: A silent drop looks the same as a message that never arrived. The sender must be able to tell that the image was not evaluated.

**Independent Test**: Set the model as unable to accept images, or leave that unset. Send one triggered image. Check that the room has one error, that no model answer appears, and that a triggered text prompt still gets an answer.

**Acceptance Scenarios**:

1. **Given** the model cannot process images, **When** a person sends one triggered image, **Then** the room shows one error that the image was not evaluated, and no model answer appears.
2. **Given** the server has not been told whether the model can process images, **When** a person sends one triggered image, **Then** the result is the same error as scenario 1.
3. **Given** the model cannot process images, **When** a person sends a triggered text prompt with no image, **Then** the agent answers as it does today.

---

### User Story 2 - Evaluate one image with the prompt (Priority: P1)

A person addresses the agent from Element with one image and a caption. The operator has said the model can process images. The model evaluates the caption and the image. The agent posts one answer in the room.

**Why this priority**: This is the capability the sender asked for. The error path is not useful unless a capable model can see the image.

**Independent Test**: Set the model as able to accept images. Send one triggered image whose contents a tester can identify. Check that the room answer refers to that image, and that the caption and the image were sent together once.

**Acceptance Scenarios**:

1. **Given** the model can process images, **When** a person sends one triggered image with a caption, **Then** the model evaluates the caption and that image together, and the room shows one answer.
2. **Given** the same capable model, **When** the image is sent in an encrypted room and again in an unencrypted room, **Then** both rooms show one answer for the same kind of image.
3. **Given** the model can process images, **When** a person sends a triggered text prompt with no image, **Then** the agent answers as it does today, with no image attached.
4. **Given** the model can process images and the image was sent, **When** the model service fails or returns no answer, **Then** the room shows one error that the model did not answer, and the caption is not sent again.

---

### User Story 3 - Reject an image the agent will not evaluate (Priority: P2)

A person addresses the agent with an image the agent will not pass on: more than one image, an image over the size limit, or an image that was read as a message but whose picture cannot be retrieved. The room shows one error. The caption is not sent for evaluation.

**Why this priority**: These cases are failures of the same promise as Story 1. They are separate because the happy path and the "model cannot see images" path can ship without them only if the limit and the single-image rule are already enforced.

**Independent Test**: With a model that can process images, use one triggered message whose content has two images, then one image over the size limit, then one triggered image whose picture cannot be retrieved. Each attempt produces one room error and no model answer. Two separate messages are two prompts. Element sends one image per message, so the two-image error is the automated check.

**Acceptance Scenarios**:

1. **Given** a triggered message that contains more than one image, **When** the agent handles it, **Then** the room shows one error that only one image is supported, and the caption is not sent for evaluation.
2. **Given** one triggered image larger than the size limit, **When** the agent handles it, **Then** the room shows one error that the image is too large, and the caption is not sent for evaluation.
3. **Given** one triggered image whose picture cannot be retrieved, **When** the agent handles it, **Then** the room shows one error that the image could not be read, and the caption is not sent for evaluation.

---

### Edge Cases

- The image caption does not address the agent. The agent ignores it, as it ignores a text message that is not a prompt. It does not send an error.
- The caption addresses the agent and the remainder is empty apart from the image. The image still counts as the prompt when the model can process images.
- The message is a file that is not an image. This version does not add a new reply. Existing behavior for that message stays as it is.
- The message cannot be decrypted, so the agent cannot tell whether it was a triggered image. The agent keeps the existing behavior for an unreadable message. It does not post a new error, because it cannot know the message was a prompt.
- The person sends a second triggered image while the first is still being evaluated. Each message gets its own answer or its own error. One failure does not cancel the other. This version is one image per message, so the second message is its own prompt.
- The image cannot be prepared for the evaluator. The room shows one error that the image could not be read. The caption is not sent on its own.
- The image was sent and the model service fails or returns no answer. The room shows one error that the model did not answer. The caption is not sent again. A text prompt that fails in this way stays on its current path.
- The model answer, or the error, must not include the image bytes, an access token, or a key.
- `BOT_ENABLED` is false. Image messages are not evaluated, and this feature posts no error.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When `BOT_ENABLED=true`, a room message that contains one image and addresses the agent with the same trigger as a text prompt MUST be treated as an image prompt. The caption after the trigger is the prompt text. The image is the attachment.
- **FR-002**: A text prompt with no image MUST keep its current behavior, including when the model cannot process images.
- **FR-003**: The system MUST pass an image prompt to the model only when the operator has explicitly said that the model can process images.
- **FR-004**: When the model cannot process images, or the operator has not said that it can, the system MUST post one error in the room and MUST NOT send that prompt for evaluation. The caption MUST NOT be sent on its own.
- **FR-005**: A room error for an image that was not sent MUST say that the image was not evaluated. It MUST NOT look like a model answer. It MUST NOT contain the image bytes, message text from other events, access tokens, or keys.
- **FR-006**: When the operator has said the model can process images, and the one image is ready, the system MUST send that image and the caption together on the existing prompt evaluation, once. When that call returns an answer, the system MUST post that answer in the room.
- **FR-007**: An image prompt in an encrypted room and an image prompt in an unencrypted room MUST produce the same kind of result for the sender.
- **FR-008**: A triggered message with more than one image MUST post one error that only one image is supported, and MUST NOT send the caption for evaluation.
- **FR-009**: A triggered image larger than 10 megabytes MUST post one error that the image is too large, and MUST NOT send the caption for evaluation.
- **FR-010**: When the message itself was read, it is one triggered image, and the picture cannot be retrieved or cannot be prepared for the evaluator, the system MUST post one error that the image could not be read, and MUST NOT send the caption for evaluation.
- **FR-011**: An image whose caption does not address the agent MUST be ignored. The system MUST NOT post an error and MUST NOT send it for evaluation.
- **FR-012**: A file that is not an image MUST NOT gain a new success path or a new error in this version.
- **FR-013**: A message that cannot be decrypted MUST keep the current unreadable-message behavior. This feature MUST NOT add a room error for it.
- **FR-014**: Logs for an image prompt MUST NOT contain image bytes, message text, access tokens, or keys.
- **FR-015**: When an image prompt was sent and the model service fails or returns no answer, the system MUST post one room error with the text `The model did not answer.` It MUST NOT send the caption again. The error MUST NOT look like a model answer and MUST NOT contain image bytes, access tokens, or keys. A text prompt with no image MUST keep its current failure behavior.

### Key Entities

- **Image prompt**: one room message that addresses the agent and carries one image. It holds the caption after the trigger and that image.
- **Model image capability**: the operator's explicit statement that the model evaluating prompts can process images. Unset means the model cannot be given an image.
- **Room error**: one message in the same room. Before the image is sent, it says the image was not evaluated, and why. After a sent image gets no answer, it says the model did not answer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For a triggered image when the model cannot process images, or when that capability is unset, the sender sees one room error within 30 seconds. No model answer appears for that message.
- **SC-002**: For a triggered text prompt with no image, the sender still receives one normal answer, including when image capability is unset.
- **SC-003**: For one triggered image at or under 10 megabytes, when the model can process images, the sender sees one room answer within 2 minutes. A tester who knows the picture can confirm the answer is about that picture.
- **SC-004**: The same image prompt produces one answer in an encrypted room and one answer in an unencrypted room. Neither room receives a second answer for that message.
- **SC-005**: Two images in one triggered message, one image over 10 megabytes, or one image that cannot be prepared, each produce one room error and no model answer. The caption for that message is not evaluated.
- **SC-006**: An image that does not address the agent produces no error and no model answer.
- **SC-007**: Across the checks above, the room text and the logs contain no image bytes, access tokens, or keys.
- **SC-008**: When a sent image prompt fails or returns no answer, the sender sees one room error that the model did not answer. No model answer appears, and the caption is not sent again.

## Assumptions

- The caption must address the agent with the same trigger already used for a text prompt. A bare image whose visible text is only a file name does not address the agent, so it is ignored.
- Element is the example client. Any Matrix client that sends one image and a caption on a single message is in scope. This version is one image per message. A second message with its own image is a separate prompt. A client that splits the caption and the image into two messages is not handled in this version. One message that contains more than one image gets the one-image error, and that case is an automated check.
- The size limit is 10 megabytes. A smaller or larger limit is an edit to FR-009 and SC-003, not a new feature.
- Image capability defaults to unset. Unset uses the room error. The operator turns it on only for a model that can process images.
- Other file types, including video, audio, and generic files, stay out of scope. They do not receive a new error.
- An undecryptable message stays on the current path from the integrated bot. This feature adds an error only after the message has been read and identified as a triggered image.
- The service that evaluates prompts already accepts one image together with the text. This was checked in Agent Zero v2.14, the version recorded for this fleet, and in the local `agent0ai/agent-zero` image at git tag `v1.9`. Both accept one image on the existing prompt evaluation. The live fleet containers were not called. This feature does not add that capability, and the fleet repo does not need a change for it.
- That evaluator still evaluates the text when an image is missing or rejected. This server refuses before the call in every case where the image will not be included.
- This server does not read the fleet model `vision` flag. The operator statement on this server is the only switch, and it defaults to unset.
- An image prompt whose caption is empty after the trigger is still one prompt. The evaluator requires some text with the image. This server sends the same prompt framing a text prompt already uses, plus the image.
- This feature does not add or change a tool. It does not change invite handling.
- The release target is the next server release, together with invite reconciliation. The version number stays unchanged until that release is frozen.
- The lab target remains `@agent0-N`, with `@agent0-5` as the concrete lab target. This feature does not use the `@agent0-2` canary exception from invite reconciliation.
