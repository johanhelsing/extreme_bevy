# Extreme Bevy

Extreme Bevy is what you end up with by following my [tutorial series on how to make a low-latency p2p web game](https://helsing.studio/posts/extreme-bevy).

There game can be played here: https://helsing.studio/extreme

It's a showcase on how to use the following together:

- [Bevy](https://github.com/bevy/bevyengine): ECS game engine for rust users.
- [GGRS](https://github.com/gschup/ggrs) for rollback networking
- [Matchbox](https://github.com/johanhelsing/matchbox) for p2p connections between browsers

## Word of caution

I intend to keep the git history of this repo as clean as possible. That means that whenever there is a new major version of one of my dependencies (or a bug fix). I'll rebase the history, instead of putting the commit at the end. That way I can easily link from the tutorial to relevant commits in the history in this repo. It also means I will force-push main and move tags around.

## License

This project is licensed under [CC0 1.0 Universal](LICENSE), except some of the contents of the [`assets`](assets) directory, see the corresponding .txt file for each assets.

I'd be happy to hear if you found it useful or made anything with it! [@jkhelsing](https://twitter.com/jkhelsing).