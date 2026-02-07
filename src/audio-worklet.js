class PcmProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    // Accumulate ~100ms of audio before posting (24000 * 0.1 = 2400 samples).
    this.buffer = new Float32Array(2400);
    this.offset = 0;
  }

  process(inputs) {
    const input = inputs[0];
    if (!input || input.length === 0) return true;
    const channel = input[0];
    if (!channel) return true;

    let src = 0;
    while (src < channel.length) {
      const space = this.buffer.length - this.offset;
      const copy = Math.min(space, channel.length - src);
      this.buffer.set(channel.subarray(src, src + copy), this.offset);
      this.offset += copy;
      src += copy;

      if (this.offset === this.buffer.length) {
        // Send a copy so the buffer can be reused immediately.
        this.port.postMessage(this.buffer.slice());
        this.offset = 0;
      }
    }

    return true;
  }
}

registerProcessor("pcm-processor", PcmProcessor);
