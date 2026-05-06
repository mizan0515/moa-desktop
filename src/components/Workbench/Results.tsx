import { useSyncExternalStore } from "react";
import SynthesisView from "../SynthesisView";
import ClaimLedger from "../ClaimLedger";
import CostMeter from "../CostMeter";
import ErrorBanner from "../ErrorBanner";
import VersionDriftWarning from "../VersionDriftWarning";
import {
  dryRunStore,
  toClaimLedger,
  toSynthesisData,
} from "../../lib/orchestrator/dryRun";

export default function Results() {
  useSyncExternalStore(dryRunStore.subscribe, dryRunStore.getSnapshot);
  const session = dryRunStore.getActive();
  const synthesis = toSynthesisData(session);
  const claims = toClaimLedger(session);

  return (
    <div>
      <VersionDriftWarning items={[]} />
      <ErrorBanner kind={null} detail={session?.errorMessage} />
      <CostMeter />
      <SynthesisView data={synthesis} />
      <ClaimLedger entries={claims} />
    </div>
  );
}
