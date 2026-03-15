import { SessionSkillsConfigurator } from "@/components/session-skills-configurator";

export default async function SessionSkillsPage({
  params,
}: {
  params: Promise<{ sessionId: string }>;
}) {
  const { sessionId } = await params;

  return <SessionSkillsConfigurator sessionId={sessionId} />;
}
