import {
  type ActivityLayout,
  type ActivityReport,
  type ActivityRound,
  type CategoryCounts,
  CLOSURE_ROUNDS,
  COUNTERS,
} from "./schema";

function sumCounts(rounds: CategoryCounts[]): CategoryCounts {
  return Object.fromEntries(
    COUNTERS.map((name) => [name, rounds.reduce((sum, round) => sum + round[name], 0)]),
  ) as CategoryCounts;
}

function nonzeroIndices(values: number[]): number[] {
  return values.flatMap((value, index) => (value > 0 ? [index] : []));
}

export function activityReport(
  words: Uint32Array,
  layout: ActivityLayout,
  totalTransportPasses: number,
  sampledTransportPasses: number,
): ActivityReport {
  if (
    words.length !== layout.words ||
    words[0] !== 1 ||
    words[1] !== 0 ||
    words[2] !== 0
  ) {
    throw new Error(
      "Transport activity capture has an invalid grid, buffer or counter overflow",
    );
  }
  if (sampledTransportPasses < 1 || sampledTransportPasses > 8) {
    throw new Error("Transport activity capture has no complete sampled passes");
  }
  const expectedVisitsPerFace = sampledTransportPasses * CLOSURE_ROUNDS;
  for (let face = 0; face < layout.interiorFaces; face += 1) {
    const observed = words[layout.visitsOffset + face];
    if (observed !== expectedVisitsPerFace) {
      throw new Error(
        `Transport face ${face} has ${observed} visits. Expected ${expectedVisitsPerFace}`,
      );
    }
  }
  const rounds: ActivityRound[] = [];
  for (let round = 0; round < CLOSURE_ROUNDS; round += 1) {
    const counts = Object.fromEntries(
      COUNTERS.map((name, index) => [
        name,
        words[layout.roundsOffset + round * COUNTERS.length + index],
      ]),
    ) as CategoryCounts;
    const validInvocations =
      counts.wall + counts.closed + counts.zeroDelta + counts.invalid + counts.writes;
    if (
      validInvocations !== layout.interiorFaces * sampledTransportPasses ||
      counts.changes > counts.writes
    ) {
      throw new Error(`Transport activity categories do not cover round ${round}`);
    }
    const tileWriteCounts = Array.from(
      words.subarray(
        layout.tileWritesOffset + round * layout.tiles,
        layout.tileWritesOffset + (round + 1) * layout.tiles,
      ),
    );
    const tileChangeCounts = Array.from(
      words.subarray(
        layout.tileChangesOffset + round * layout.tiles,
        layout.tileChangesOffset + (round + 1) * layout.tiles,
      ),
    );
    if (tileChangeCounts.some((count, tile) => count > tileWriteCounts[tile])) {
      throw new Error(`Transport tile changes exceed writes in round ${round}`);
    }
    for (const [tileCounts, expected] of [
      [tileWriteCounts, counts.writes],
      [tileChangeCounts, counts.changes],
    ] as const) {
      const endpoints = tileCounts.reduce((sum, count) => sum + count, 0);
      if (endpoints < expected || endpoints > expected * 2) {
        throw new Error(
          `Transport tile totals do not cover face endpoints in round ${round}`,
        );
      }
    }
    rounds.push({
      round,
      ...counts,
      validInvocations,
      tileWriteCounts,
      tileChangeCounts,
      activeWriteTiles: nonzeroIndices(tileWriteCounts),
      activeChangeTiles: nonzeroIndices(tileChangeCounts),
    });
  }
  const activeChangeTiles = [
    ...new Set(rounds.flatMap((round) => round.activeChangeTiles)),
  ].sort((a, b) => a - b);
  const firstRoundChanges = new Set(rounds[0].activeChangeTiles);
  return {
    schemaVersion: 1,
    measurement:
      "Diagnostic shader counters. Instrumented duration cannot establish a performance gain",
    grid: { width: layout.width, height: layout.height },
    tiles: {
      size: layout.tileSize,
      columns: layout.tileColumns,
      rows: layout.tileRows,
      count: layout.tiles,
    },
    closureRounds: CLOSURE_ROUNDS,
    interiorFaces: layout.interiorFaces,
    totalTransportPasses,
    sampledTransportPasses,
    expectedVisitsPerFace,
    verifiedFaces: layout.interiorFaces,
    validInvocations: rounds.reduce((sum, round) => sum + round.validInvocations, 0),
    totals: sumCounts(rounds),
    activeWriteTiles: [
      ...new Set(rounds.flatMap((round) => round.activeWriteTiles)),
    ].sort((a, b) => a - b),
    activeChangeTiles,
    firstRoundInactiveTilesThatLaterChange: activeChangeTiles.filter(
      (tile) => !firstRoundChanges.has(tile),
    ),
    rounds,
    limitations: [
      "Total transport passes count passes started while armed. The sample limit selects the first passes in that window",
      "Counts aggregate the selected transport candidate passes, including any rejected candidate",
      "Writes count executions that assign velocity and residuals. Changes require at least one different f32 bit pattern",
      "Each affected endpoint tile is counted once per face. A cross-tile face contributes to two tile counters",
      "No-write counts do not establish removable validation. Zero-delta faces can become active in later parity passes or rounds",
      "The observer covers local residual pairs only. It does not measure pressure propagation or all transport stages",
    ],
  };
}
