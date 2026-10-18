import { useState } from 'react';

/** The homepage's amber band wipe, once, as the page arrives. The app runs the same one. */
export function Eyecatch() {
	const [done, setDone] = useState(false);
	if (done) return null;
	return <div className="eyecatch" aria-hidden="true" onAnimationEnd={() => setDone(true)} />;
}
