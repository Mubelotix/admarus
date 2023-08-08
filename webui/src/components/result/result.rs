use crate::prelude::*;

#[derive(Clone, Properties)]
pub struct GroupedResultsProps {
    pub results: Vec<(DocumentResult, Scores)>,
    pub connection_status: Rc<ConnectionStatus>,
    pub query: Rc<Query>,
}

// TODO: revise this
impl PartialEq for GroupedResultsProps {
    fn eq(&self, other: &Self) -> bool {
        if self.results.len() != other.results.len() {
            return false;
        }
        if self.query != other.query {
            return false;
        }
        for i in 0..self.results.len() {
            if self.results[i].0.cid != other.results[i].0.cid {
                return false;
            }
            if self.results[i].1 != other.results[i].1 {
                return false;
            }
        }
        true
    }
}

pub struct GroupedResultsComp {
    displayed: usize,
}

impl Component for GroupedResultsComp {
    type Message = ();
    type Properties = GroupedResultsProps;

    fn create(_ctx: &Context<Self>) -> Self {
        GroupedResultsComp {
            displayed: 3,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, _msg: Self::Message) -> bool {
        self.displayed += 5;
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if ctx.props().results.is_empty() {
            return html! {};
        }
        let all_displayed = self.displayed >= ctx.props().results.len();
        let root_id = ctx.props().results.first().map(|(result,_)| result.root_id()).unwrap_or_default();
        let conn_status = &ctx.props().connection_status;

        let result_iter = || ctx.props().results.iter().take(self.displayed + 1).map(|(result,_)| result);
        let favicon_iter = || ctx.props().results.first().unwrap().0.favicons.iter();
        let scores_iter = || ctx.props().results.iter().take(self.displayed + 1).map(|(_,scores)| scores);

        // General
        let mut href_iter = result_iter().map(|result| result.format_best_href(conn_status));
        let mut title_iter = result_iter().map(|result| result.format_result_title());
        let mut desc_iter = result_iter().map(|result| result.view_desc(&ctx.props().query));
        
        let href_first = href_iter.next().unwrap_or_default();
        let title_first = title_iter.next().unwrap_or_default();
        let desc_first = desc_iter.next().unwrap_or_default();
        let addr_first = ctx.props().results.first().unwrap().0.format_best_addr();

        // Favicons
        let icon_sizes_iter = favicon_iter().map(|desc| desc.sizes.to_owned());
        let icon_srcset_iter = favicon_iter().map(|desc| desc.format_srcset(ctx.props().results.first().unwrap().0.paths.first(), conn_status));
        let icon_type_iter = favicon_iter().map(|desc| desc.mime_type.to_owned());

        // Scores
        let display_scores = false; // cfg!(debug_assertions);
        let mut term_frequency_score_iter = scores_iter().map(|scores| scores.tf_score);
        let mut variety_score_iter = scores_iter().map(|scores| scores.variety_score);
        let mut length_score_iter = scores_iter().map(|scores| scores.length_score);
        let mut lang_score_iter = scores_iter().map(|scores| scores.lang_score);
        let mut popularity_score_iter = scores_iter().map(|scores| scores.popularity_score);
        let mut verified_score_iter = scores_iter().map(|scores| scores.verified_score);

        let term_frequency_score_first = term_frequency_score_iter.next().unwrap();
        let variety_score_first = variety_score_iter.next().unwrap();
        let length_score_first = length_score_iter.next().unwrap();
        let lang_score_first = lang_score_iter.next().unwrap();
        let popularity_score_first = popularity_score_iter.next().unwrap();
        let verified_score_first = verified_score_iter.next().unwrap();

        // Events
        let onclick_more = ctx.link().callback(|_| ());

        template_html!("components/result/result.html", ...)
    }
}
