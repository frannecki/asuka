import { SessionMemoryView } from "@/components/session-memory-view";

export default async function SessionMemoryPage({
  params,
}: {
  params: Promise<{ sessionId: string }>;
}) {
  const { sessionId } = await params;

  return <SessionMemoryView sessionId={sessionId} />;
}
