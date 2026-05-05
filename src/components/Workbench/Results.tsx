import SynthesisView from "../SynthesisView";
import ClaimLedger from "../ClaimLedger";
import CostMeter from "../CostMeter";
import ErrorBanner from "../ErrorBanner";

export default function Results() {
  return (
    <div>
      <ErrorBanner />
      <CostMeter />
      <SynthesisView />
      <ClaimLedger />
    </div>
  );
}
