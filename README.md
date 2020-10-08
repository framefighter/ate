# ate

save, create, plan, and admire your meals with telegram.

## plan

### v0.1.0

- [X] basic interaction (buttons, commands, inline)

### v0.2.0

- [X] save meals `/new <name> [, rating (number)] [, tags (separated with spaces)] [, links or references]`
- [X] basic step by step creating (only rating supported) `/newmeal <name>`
- [X] basic inline support (list and creation) `@<bot-name> Search Meals...`
- [X] get meal `/get <name>`
- [X] list meals `/list`
- [X] plan meals `/plan <number>` (uses rating as weight)

### v0.3.0

- [ ] group polling for meal rating
- [ ] whitelist users

### future releases

- [ ] make db chat/group exclusive
- [ ] handle multiple meals with same name
- [ ] edit entries after creation
- [ ] support multiple pictures per meal
- [ ] expand step by step creation
- [ ] more sophisticated planning (tag variety and frequenzy of meals)
- [ ] advanced error handling
