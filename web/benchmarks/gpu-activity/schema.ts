export const CLOSURE_ROUNDS = 256;
export const ACTIVITY_LABEL = "transport local residual pairs";
export const COUNTERS = ["wall", "closed", "zeroDelta", "invalid", "writes", "changes"] as const;
export type CategoryCounts = Record<(typeof COUNTERS)[number], number>;

export type ActivityLayout = {
	width: number;
	height: number;
	tileSize: number;
	tileColumns: number;
	tileRows: number;
	tiles: number;
	uInteriorFaces: number;
	interiorFaces: number;
	visitsOffset: number;
	roundsOffset: number;
	tileWritesOffset: number;
	tileChangesOffset: number;
	words: number;
};

export function activityLayout(options: {
	width: number;
	height: number;
	tileSize?: number;
}): ActivityLayout {
	const { width, height, tileSize = 16 } = options;
	if (width !== 480 || height !== 270 || tileSize !== 16) {
		throw new Error("Transport activity capture requires the 480 × 270 grid and 16 × 16 tiles");
	}
	const tileColumns = Math.ceil(width / tileSize);
	const tileRows = Math.ceil(height / tileSize);
	const tiles = tileColumns * tileRows;
	const uInteriorFaces = (width - 1) * height;
	const interiorFaces = uInteriorFaces + width * (height - 1);
	const visitsOffset = 4;
	const roundsOffset = visitsOffset + interiorFaces;
	const tileWritesOffset = roundsOffset + CLOSURE_ROUNDS * COUNTERS.length;
	const tileChangesOffset = tileWritesOffset + CLOSURE_ROUNDS * tiles;
	return {
		width,
		height,
		tileSize,
		tileColumns,
		tileRows,
		tiles,
		uInteriorFaces,
		interiorFaces,
		visitsOffset,
		roundsOffset,
		tileWritesOffset,
		tileChangesOffset,
		words: tileChangesOffset + CLOSURE_ROUNDS * tiles,
	};
}

export type ActivityRound = CategoryCounts & {
	round: number;
	validInvocations: number;
	activeWriteTiles: number[];
	activeChangeTiles: number[];
	tileWriteCounts: number[];
	tileChangeCounts: number[];
};

export type ActivityReport = {
	schemaVersion: 1;
	measurement: string;
	grid: { width: number; height: number };
	tiles: { size: number; columns: number; rows: number; count: number };
	closureRounds: number;
	interiorFaces: number;
	totalTransportPasses: number;
	sampledTransportPasses: number;
	expectedVisitsPerFace: number;
	verifiedFaces: number;
	validInvocations: number;
	totals: CategoryCounts;
	activeWriteTiles: number[];
	activeChangeTiles: number[];
	firstRoundInactiveTilesThatLaterChange: number[];
	rounds: ActivityRound[];
	limitations: string[];
};
