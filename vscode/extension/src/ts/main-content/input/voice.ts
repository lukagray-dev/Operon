// Voice Input & Speech-to-Text Recognition Coordinator for VS Code

import { showAlertDialog, showPermissionDialog } from '../../shared/dialog.js';
import { inputState } from './state.js';
import type { ISpeechRecognition, SpeechRecognitionErrorEvent, SpeechRecognitionEvent } from './types.js';

let recognitionInstance: ISpeechRecognition | null = null;
let preVoiceText = '';

export function isVoiceSupported(): boolean {
  const win = window as unknown as {
    SpeechRecognition?: new () => ISpeechRecognition;
    webkitSpeechRecognition?: new () => ISpeechRecognition;
  };
  return Boolean(win.SpeechRecognition || win.webkitSpeechRecognition);
}

export async function toggleVoiceRecording(): Promise<void> {
  if (inputState.getIsReadOnly()) {
    return;
  }

  if (inputState.getIsVoiceRecording()) {
    stopVoiceRecording();
  } else {
    await startVoiceRecording();
  }
}

export async function startVoiceRecording(): Promise<void> {
  const win = window as unknown as {
    SpeechRecognition?: new () => ISpeechRecognition;
    webkitSpeechRecognition?: new () => ISpeechRecognition;
  };

  const SpeechRecognitionClass = win.SpeechRecognition || win.webkitSpeechRecognition;

  if (!SpeechRecognitionClass) {
    console.error('[Voice] Speech recognition is not supported in this webview environment.');
    await showAlertDialog({
      title: 'Voice Input Unsupported',
      message: 'Speech recognition is not supported in this environment. Please check your system microphone permissions.',
      buttonText: 'Ok',
      icon: 'warning',
    });
    return;
  }

  const hasGranted = localStorage.getItem('operon_mic_permission_granted') === 'true';
  if (!hasGranted) {
    const userAllowed = await showPermissionDialog({
      title: 'Microphone Permission',
      message: 'Operon needs access to your microphone for real-time speech-to-text typing.',
      allowText: 'Ok',
      denyText: 'Cancel',
      icon: 'mic',
    });

    if (!userAllowed) {
      console.debug('[Voice] User denied microphone access in custom dialog.');
      return;
    }

    try {
      if (navigator.mediaDevices && navigator.mediaDevices.getUserMedia) {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        stream.getTracks().forEach((track) => track.stop());
      }
      localStorage.setItem('operon_mic_permission_granted', 'true');
    } catch (permErr) {
      console.warn('[Voice] Media devices permission request error:', permErr);
      await showAlertDialog({
        title: 'Microphone Access Denied',
        message: 'Could not access the microphone. Please check your Windows microphone privacy settings.',
        buttonText: 'Ok',
        icon: 'danger',
      });
      return;
    }
  }

  const textarea = document.getElementById('chat-input-textarea') as HTMLTextAreaElement | null;
  const voiceBtn = document.getElementById('btn-voice-input');

  if (!recognitionInstance) {
    try {
      recognitionInstance = new SpeechRecognitionClass();
      recognitionInstance.continuous = false;
      recognitionInstance.interimResults = true;
      recognitionInstance.lang = navigator.language || 'en-US';

      recognitionInstance.onstart = () => {
        inputState.setIsVoiceRecording(true);
        preVoiceText = textarea ? textarea.value : '';
        if (voiceBtn) {
          voiceBtn.setAttribute('title', 'Stop voice typing');
        }
        console.debug('[Voice] Speech recognition session started.');
      };

      recognitionInstance.onresult = (event: SpeechRecognitionEvent) => {
        if (!textarea) return;

        let interimTranscript = '';
        let finalTranscript = '';

        for (let i = event.resultIndex; i < event.results.length; ++i) {
          const segment = event.results[i][0].transcript;
          if (event.results[i].isFinal) {
            finalTranscript += segment;
          } else {
            interimTranscript += segment;
          }
        }

        const base = preVoiceText || '';
        const spacing = base.length > 0 && (finalTranscript.length > 0 || interimTranscript.length > 0) ? ' ' : '';
        const combinedText = base + spacing + finalTranscript + interimTranscript;

        textarea.value = combinedText;
        inputState.setInputText(combinedText);

        textarea.style.height = 'auto';
        const newHeight = Math.min(200, Math.max(42, textarea.scrollHeight));
        textarea.style.height = `${newHeight}px`;
      };

      recognitionInstance.onerror = async (event: SpeechRecognitionErrorEvent) => {
        console.warn('[Voice] Speech recognition error:', event.error);

        if (event.error === 'not-allowed') {
          await showAlertDialog({
            title: 'Microphone Access Denied',
            message: 'Microphone access was denied. Please grant microphone permissions to Operon in your Windows settings.',
            buttonText: 'Ok',
            icon: 'danger',
          });
        } else if (event.error !== 'no-speech') {
          console.error(`[Voice] Transcription error: ${event.error}`);
        }

        cleanupVoiceUI();
      };

      recognitionInstance.onend = () => {
        cleanupVoiceUI();
        console.debug('[Voice] Speech recognition session ended.');
      };
    } catch (err) {
      console.error('[Voice] Failed to initialize SpeechRecognition:', err);
      await showAlertDialog({
        title: 'Voice Configuration Error',
        message: 'Could not configure the voice speech recognition interface.',
        buttonText: 'Ok',
        icon: 'danger',
      });
      return;
    }
  }

  try {
    recognitionInstance.start();
  } catch (err) {
    console.warn('[Voice] Speech recognition start warning (already active or busy):', err);
  }
}

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

function cleanupVoiceUI(): void {
  inputState.setIsVoiceRecording(false);
  const voiceBtn = document.getElementById('btn-voice-input');
  if (voiceBtn) {
    voiceBtn.setAttribute('title', 'Voice input');
  }
}
