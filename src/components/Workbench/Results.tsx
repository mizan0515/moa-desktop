import SynthesisView from "../SynthesisView";
import ClaimLedger from "../ClaimLedger";
import CostMeter from "../CostMeter";
import ErrorBanner from "../ErrorBanner";
import VersionDriftWarning from "../VersionDriftWarning";

export default function Results() {
  // T9: real telemetry / error / drift data is wired by T7 orchestrator.
  // Until then, render with empty defaults so the layout settles.
  return (
    <div>
      <VersionDriftWarning items={[]} />
      <ErrorBanner kind={null} />
      <CostMeter />
      <SynthesisView />
      <ClaimLedger />
    </div>
  );
}
