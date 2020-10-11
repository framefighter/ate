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

- [X] remove meals per command `/remove <name>`
- [X] group polling for meal rating
- [X] whitelist users `/op <username> <password (from config)>`
- [X] (handle multiple meals with same name)

#### v0.3.1

- [X] removed delete button
- [X] improved poll flow

### v0.4.0

- [X] rework state
- [X] create backups on start
- [X] edit entries after creation

#### v0.4.1

- [X] edit photos
- [X] sending meal with photos (improved)

##### bugs

- [ ] Keyboards not getting removed consistently

### future releases

- [ ] make db chat/group exclusive
- [ ] database migration
- [ ] handle multiple meals with same name v2
- [ ] support multiple pictures per meal
- [ ] expand step by step creation
- [ ] more sophisticated planning (tag variety and frequenzy of meals)
- [ ] advanced error handling
