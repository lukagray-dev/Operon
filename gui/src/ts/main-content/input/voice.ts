// Voice Input & Speech-to-Text Recognition Coordinator
//
// This module provides speech recognition capabilities inside the main chat prompt input.
// It leverages the standard Web Speech API (SpeechRecognition / webkitSpeechRecognition)
// built natively into the Chromium / Webview2 engine on Windows.

import { inputState } from './state.js';
import type { ISpeechRecognition, SpeechRecognitionErrorEvent, SpeechRecognitionEvent } from './types.js';

// Holds the active SpeechRecognition instance once initialized
let recognitionInstance: ISpeechRecognition | null = null;

// Stores whatever text was in the textarea before the user pressed the voice button,
// so speech-to-text appends nicely to existing prompt text without overwriting it!
let preVoiceText = '';

/**
 * Checks if Speech Recognition is supported in the current browser/webview environment.
 */
export function isVoiceSupported(): boolean {
  const win = window as unknown as {
    SpeechRecognition?: new () => ISpeechRecognition;
    webkitSpeechRecognition?: new () => ISpeechRecognition;
  };
  return Boolean(win.SpeechRecognition || win.webkitSpeechRecognition);
}

/**
 * Toggles voice recording on/off when the user clicks the microphone button.
 */
export function toggleVoiceRecording(): void {
  // If the conversation is currently read-only, do not allow recording
  if (inputState.getIsReadOnly()) {
    return;
  }

  // If we are already recording, clicking the button again stops the session
  if (inputState.getIsVoiceRecording()) {
    stopVoiceRecording();
  } else {
    startVoiceRecording();
  }
}

/**
 * Starts speech recognition and streams live transcripts into the prompt textarea.
 */
export function startVoiceRecording(): void {
  const win = window as unknown as {
    SpeechRecognition?: new () => ISpeechRecognition;
    webkitSpeechRecognition?: new () => ISpeechRecognition;
  };

  const SpeechRecognitionClass = win.SpeechRecognition || win.webkitSpeechRecognition;

  if (!SpeechRecognitionClass) {
    console.error('[Voice] Speech recognition is not supported in this webview environment.');
    alert('Speech recognition is not supported in this browser environment. Please check your system microphone permissions.');
    return;
  }

  const textarea = document.getElementById('chat-input-textarea') as HTMLTextAreaElement | null;
  const voiceBtn = document.getElementById('btn-voice-input');

  // Initialize recognition instance if not created yet
  if (!recognitionInstance) {
    try {
      recognitionInstance = new SpeechRecognitionClass();
      // continuous = false stops listening automatically when the speaker pauses
      recognitionInstance.continuous = false;
      // interimResults = true allows live preview of words as they are spoken
      recognitionInstance.interimResults = true;
      recognitionInstance.lang = navigator.language || 'en-US';

      // 1. When the microphone starts capturing audio
      recognitionInstance.onstart = () => {
        inputState.setIsVoiceRecording(true);
        // Save the baseline text in the textarea so speech appends after it
        preVoiceText = textarea ? textarea.value : '';
        if (voiceBtn) {
          voiceBtn.setAttribute('title', 'Stop voice typing');
        }
        console.debug('[Voice] Speech recognition session started.');
      };

      // 2. When partial or final speech results arrive from the speech engine
      recognitionInstance.onresult = (event: SpeechRecognitionEvent) => {
        if (!textarea) return;

        let interimTranscript = '';
        let finalTranscript = '';

        // Iterate through all incoming speech segments
        for (let i = event.resultIndex; i < event.results.length; ++i) {
          const segment = event.results[i][0].transcript;
          if (event.results[i].isFinal) {
            finalTranscript += segment;
          } else {
            interimTranscript += segment;
          }
        }

        // Combine existing pre-voice text with the new transcripts
        const base = preVoiceText || '';
        const spacing = base.length > 0 && (finalTranscript.length > 0 || interimTranscript.length > 0) ? ' ' : '';
        const combinedText = base + spacing + finalTranscript + interimTranscript;

        // Update the textarea and reactive input state
        textarea.value = combinedText;
        inputState.setInputText(combinedText);

        // Adjust textarea height dynamically to fit the growing text
        textarea.style.height = 'auto';
        const newHeight = Math.min(200, Math.max(42, textarea.scrollHeight));
        textarea.style.height = `${newHeight}px`;
      };

      // 3. When an error occurs during audio capture or transcription
      recognitionInstance.onerror = (event: SpeechRecognitionErrorEvent) => {
        console.warn('[Voice] Speech recognition error:', event.error);

        // Handle blocked permission scenarios specifically
        if (event.error === 'not-allowed') {
          alert('Microphone access was denied. Please grant microphone permissions to Operon in your Windows settings.');
        } else if (event.error !== 'no-speech') {
          // Suppress alert for simple silence / no-speech timeouts
          console.error(`[Voice] Transcription error: ${event.error}`);
        }

        cleanupVoiceUI();
      };

      // 4. When the recognition session naturally concludes (speaker stops or manual stop)
      recognitionInstance.onend = () => {
        cleanupVoiceUI();
        console.debug('[Voice] Speech recognition session ended.');
      };
    } catch (err) {
      console.error('[Voice] Failed to initialize SpeechRecognition:', err);
      alert('Could not configure voice interface.');
      return;
    }
  }

  // Start listening to the microphone
  try {
    recognitionInstance.start();
  } catch (err) {
    console.warn('[Voice] Speech recognition start warning (already active or busy):', err);
  }
}

/**
 * Manually stops the active speech recognition session.
 */
export function stopVoiceRecording(): void {
  if (recognitionInstance) {
    try {
      recognitionInstance.stop();
    } catch (err) {
      console.warn('[Voice] Failed to stop recognition instance gracefully:', err);
    }
  }
  cleanupVoiceUI();
}

/**
 * Helper to reset UI states, reactive flags, and button titles.
 */
function cleanupVoiceUI(): void {
  inputState.setIsVoiceRecording(false);
  const voiceBtn = document.getElementById('btn-voice-input');
  if (voiceBtn) {
    voiceBtn.setAttribute('title', 'Voice input');
  }
}
