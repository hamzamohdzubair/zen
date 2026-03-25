# Web Search Setup Guide

The LLM question generator can now use web search to access the latest information when creating questions. This ensures questions are based on current, up-to-date knowledge rather than just the LLM's training data.

## How It Works

When generating questions, the LLM can decide to search the web if it needs:
- Recent information about rapidly changing topics
- Latest developments in technology, science, or current events
- Up-to-date best practices and standards
- Current versions of tools, frameworks, or libraries

The LLM intelligently decides when to search - for evergreen topics like "What is a variable?" it won't search, but for "Latest React hooks patterns" it will.

## Supported Search Providers

Choose one of these providers:

### 1. Tavily (Recommended)
- **Best for**: General use, free tier available
- **Free tier**: 1,000 searches/month
- **Signup**: https://tavily.com
- **Pros**: Purpose-built for AI, returns well-formatted results

### 2. Brave Search
- **Best for**: Privacy-focused searches
- **Free tier**: 2,000 queries/month
- **Signup**: https://brave.com/search/api/
- **Pros**: No tracking, fast results

### 3. Serper
- **Best for**: Google-powered results
- **Free tier**: 2,500 queries
- **Signup**: https://serper.dev
- **Pros**: Uses Google Search, comprehensive results

### 4. SerpAPI
- **Best for**: Advanced use cases
- **Free tier**: 100 searches/month
- **Signup**: https://serpapi.com
- **Pros**: Many features, well-documented

## Configuration

Add the web search configuration to your `~/.zen/config.toml`:

```toml
[llm]
provider = "groq"
api_key = "your-groq-api-key"
model = "llama-3.3-70b-versatile"

[web_search]
provider = "tavily"  # or "brave", "serper", "serpapi"
api_key = "your-search-api-key"
```

## Setup Instructions

### Step 1: Choose a Provider
Pick one of the providers above based on your needs and sign up for an API key.

### Step 2: Get API Key
After signing up:
- **Tavily**: Dashboard → API Keys
- **Brave**: Developer Dashboard → API Keys
- **Serper**: Dashboard → API Key
- **SerpAPI**: Dashboard → Your Private API Key

### Step 3: Update Config
Edit `~/.zen/config.toml` and add the `[web_search]` section with your chosen provider and API key.

### Step 4: Test
Run `zen review` and create a topic with current technology keywords (e.g., "React 19", "Rust async", "Python 3.13"). The LLM should use web search to generate up-to-date questions.

## Example Configurations

### Tavily (Recommended)
```toml
[web_search]
provider = "tavily"
api_key = "tvly-xxxxxxxxxxxxxxxxxxxxx"
```

### Brave Search
```toml
[web_search]
provider = "brave"
api_key = "BSAxxxxxxxxxxxxxxxxxxxxx"
```

### Serper
```toml
[web_search]
provider = "serper"
api_key = "xxxxxxxxxxxxxxxxxxxxxxxx"
```

### SerpAPI
```toml
[web_search]
provider = "serpapi"
api_key = "xxxxxxxxxxxxxxxxxxxxxxxx"
```

## Optional Setup

Web search is **optional**. If you don't configure it, question generation will work as before using only the LLM's training data.

To disable web search, simply remove or comment out the `[web_search]` section in your config.

## Troubleshooting

### "Web search not configured" error
The LLM tried to use web search but it's not set up. Add the `[web_search]` section to your config.

### "API returned status 401"
Your API key is invalid or expired. Check your provider's dashboard and update the key in your config.

### "API returned status 429"
You've exceeded your rate limit. Wait for it to reset or upgrade your plan.

### Questions seem outdated
The LLM decided not to search. This is normal for evergreen topics. For time-sensitive topics, ensure your keywords suggest recent information (e.g., include year, version numbers, or "latest").

## Cost Considerations

All providers offer free tiers sufficient for typical use:
- **Question generation typically uses 1-3 searches per topic**
- **Most free tiers allow 1,000+ searches/month**
- **For 10 reviews/day with 3 questions each, that's ~90 searches/month**

If you approach limits, consider:
1. Using web search only for time-sensitive topics
2. Upgrading to a paid plan (typically $5-20/month)
3. Switching providers to combine free tiers

## Privacy Note

Search queries include your topic keywords. If you're studying sensitive material:
- Use Brave Search (no tracking)
- Review your provider's privacy policy
- Consider disabling web search for sensitive topics

## Technical Details

### How Tool Calling Works
1. LLM receives question generation request
2. LLM decides if web search is needed
3. If yes, LLM calls `web_search` function with query
4. Search results are fetched from your configured provider
5. LLM uses results to generate an informed question

### Search Result Format
The search returns 3 top results with titles and snippets, plus a summary (when available). This gives the LLM enough context without overwhelming it.

### Performance
- Search adds 0.5-2 seconds to question generation
- Results are worth the wait for time-sensitive topics
- Groq's fast inference minimizes overall impact

## Future Enhancements

Potential improvements we're considering:
- Caching search results to reduce API calls
- User-controlled search triggers (always/never/auto)
- Multiple sources per question
- Citation links in generated questions

Have suggestions? Open an issue on GitHub!
