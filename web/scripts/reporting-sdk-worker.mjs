import { createSentrySDK } from "sentry";

const resultPrefix = "PARTICLE_FOUNDRY_SENTRY_RESULT=";

try {
  let input = "";
  for await (const chunk of process.stdin) {
    input += chunk;
  }
  const request = JSON.parse(input);
  const sdk = createSentrySDK(request.options);
  const result =
    request.operation === "inject"
      ? await sdk.sourcemap.inject(request.parameters)
      : await sdk.sourcemap.upload(request.parameters);
  process.stdout.write(`${resultPrefix}${JSON.stringify(result)}\n`);
} catch {
  process.exitCode = 1;
}
