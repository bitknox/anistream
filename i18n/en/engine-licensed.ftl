# Notice when the viewer comes back from a licensed service after long enough to have watched an episode. It holds Mark as watched for 8 seconds. The service is the one they opened, such as Hikari Stream.
# @arg episode: int
# @arg service: text
licensed-return = Watched episode { $episode } on { $service }?

# Notice after Mark as watched on the return notice. It holds Undo.
# @arg episode: int
licensed-marked = Episode { $episode } marked as watched.
