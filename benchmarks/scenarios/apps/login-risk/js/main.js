import emotionHash from "@emotion/hash";
import clamp from "clamp";
import levenshtein from "js-levenshtein";
import stringHash from "string-hash";

const attempts = [
  ["ada@example.test", "compile!42", "compiler!24"],
  ["grace@example.test", "navy-cobol", "navy-cobalt"],
  ["linus@example.test", "penguin-kernel", "kernel-penguin"],
  ["margaret@example.test", "apollo-guidance", "apollo-guide"],
];

const audits = [];
let digest = 0;
for (let round = 0; round < 750; round += 1) {
  for (const [email, password, previous] of attempts) {
    const distance = levenshtein(password, previous);
    const _riskScore = clamp(distance * 13 + (stringHash(email) % 31), 0, 100);
    const _fingerprint = emotionHash(`${email}:${password}`);
    audits.push({ _riskScore, _fingerprint });
    digest = (digest + _riskScore * 17 + _fingerprint.length * 29) % 2147483647;
  }
}

const last = audits[audits.length - 1];
console.log(`login-risk:${audits.length}:${digest}:${last._riskScore}:${last._fingerprint}`);
