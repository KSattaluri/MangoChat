class PcmProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
  }

  process(inputs) {
    const input = inputs[0];
    if (!input || input.length === 0) {
      return true;
    }
    const channel = input[0];
    if (!channel) {
      return true;
    }
    this.port.postMessage(channel);
    return true;
  }
}

registerProcessor("pcm-processor", PcmProcessor);
