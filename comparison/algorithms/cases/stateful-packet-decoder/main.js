import { decodePacket } from "./decoder.js";

function runPacketDecoder() {
  return decodePacket(algorithmString(0));
}

console.log(runPacketDecoder());
