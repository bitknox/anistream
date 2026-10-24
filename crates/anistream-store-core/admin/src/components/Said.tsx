/**
 * A sentence from the store, with what it puts in backticks set as code: its errors name the
 * command that fixes them, such as `anistream plugin stamp`, and that should read as one.
 */
export function Said({ text }: { text: string }) {
	const pieces = text.split('`');
	// An odd count of backticks would leave one unpaired; show the words as they came.
	if (pieces.length % 2 === 0) return <>{text}</>;
	return (
		<>
			{pieces.map((piece, i) =>
				// The pieces never move: their place in the sentence is who they are.
				// biome-ignore lint/suspicious/noArrayIndexKey: a fixed split of one string
				i % 2 ? <code key={i}>{piece}</code> : piece,
			)}
		</>
	);
}
